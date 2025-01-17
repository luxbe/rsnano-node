#include "nano/lib/rsnano.hpp"

#include <nano/lib/threading.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/node.hpp>
#include <nano/secure/store.hpp>

#include <boost/format.hpp>

namespace nano
{
class block_processor_lock
{
public:
	block_processor_lock (rsnano::BlockProcessorHandle * handle_a) :
		handle{ rsnano::rsn_block_processor_lock (handle_a) }
	{
	}
	block_processor_lock (block_processor_lock const &) = delete;
	~block_processor_lock ()
	{
		rsnano::rsn_block_processor_lock_destroy (handle);
	}
	void lock (rsnano::BlockProcessorHandle * processor)
	{
		rsnano::rsn_block_processor_lock_lock (handle, processor);
	}
	void unlock ()
	{
		rsnano::rsn_block_processor_lock_unlock (handle);
	}
	rsnano::BlockProcessorLockHandle * handle;
};
}

nano::block_processor::block_processor (nano::node & node_a, nano::write_database_queue & write_database_queue_a) :
	next_log (std::chrono::steady_clock::now ()),
	logger (*node_a.logger),
	checker (node_a.checker),
	config (*node_a.config),
	state_block_signature_verification (checker, config.network_params.ledger.epochs, config.logging.timing_logging (), node_a.logger, node_a.flags.block_processor_verification_size ()),
	network_params (node_a.network_params),
	ledger (node_a.ledger),
	flags (node_a.flags),
	store (node_a.store),
	stats (*node_a.stats),
	block_arrival (node_a.block_arrival),
	unchecked (node_a.unchecked),
	gap_cache (node_a.gap_cache),
	write_database_queue (write_database_queue_a)
{
	blocks_rolled_back =
	[&node_a] (std::vector<std::shared_ptr<nano::block>> const & rolled_back, std::shared_ptr<nano::block> const & initial_block) {
		// Deleting from votes cache, stop active transaction
		for (auto & i : rolled_back)
		{
			node_a.history.erase (i->root ());
			// Stop all rolled back active transactions except initial
			if (i->hash () != initial_block->hash ())
			{
				node_a.active.erase (*i);
			}
		}
	};

	handle = rsnano::rsn_block_processor_create (this);

	batch_processed.add ([this] (auto const & items) {
		// For every batch item: notify the 'processed' observer.
		for (auto const & item : items)
		{
			auto const & [result, block] = item;
			processed.notify (result, block);
		}
	});
	blocking.connect (*this);
	state_block_signature_verification.blocks_verified_callback = [this] (std::deque<nano::state_block_signature_verification::value_type> & items, std::vector<int> const & verifications, std::vector<nano::block_hash> const & hashes, std::vector<nano::signature> const & blocks_signatures) {
		this->process_verified_state_blocks (items, verifications, hashes, blocks_signatures);
	};
	state_block_signature_verification.transition_inactive_callback = [this] () {
		if (this->flushing)
		{
			{
				// Prevent a race with condition.wait in block_processor::flush
				nano::block_processor_lock guard{ this->handle };
			}
			rsnano::rsn_block_processor_notify_all (this->handle);
		}
	};
}

nano::block_processor::~block_processor ()
{
	rsnano::rsn_block_processor_destroy (handle);
}

rsnano::BlockProcessorHandle const * nano::block_processor::get_handle () const
{
	return handle;
}

void nano::block_processor::start ()
{
	processing_thread = std::thread ([this] () {
		nano::thread_role::set (nano::thread_role::name::block_processing);
		this->process_blocks ();
	});
}

void nano::block_processor::stop ()
{
	{
		nano::block_processor_lock lock{ handle };
		stopped = true;
	}
	rsnano::rsn_block_processor_notify_all (handle);
	blocking.stop ();
	state_block_signature_verification.stop ();
	nano::join_or_pass (processing_thread);
}

void nano::block_processor::flush ()
{
	checker.flush ();
	flushing = true;
	nano::block_processor_lock lock{ handle };
	while (!stopped && (have_blocks () || active || state_block_signature_verification.is_active ()))
	{
		rsnano::rsn_block_processor_wait (handle, lock.handle);
	}
	flushing = false;
}

std::size_t nano::block_processor::size ()
{
	nano::block_processor_lock lock{ handle };
	return (blocks.size () + state_block_signature_verification.size () + forced.size ());
}

bool nano::block_processor::full ()
{
	return size () >= flags.block_processor_full_size ();
}

bool nano::block_processor::half_full ()
{
	return size () >= flags.block_processor_full_size () / 2;
}

void nano::block_processor::process_active (std::shared_ptr<nano::block> const & incoming)
{
	block_arrival.add (incoming->hash ());
	add (incoming);
}

void nano::block_processor::add (std::shared_ptr<nano::block> const & block)
{
	if (full ())
	{
		stats.inc (nano::stat::type::blockprocessor, nano::stat::detail::overfill);
		return;
	}
	if (network_params.work.validate_entry (*block)) // true => error
	{
		stats.inc (nano::stat::type::blockprocessor, nano::stat::detail::insufficient_work);
		return;
	}
	add_impl (block);
	return;
}

std::optional<nano::process_return> nano::block_processor::add_blocking (std::shared_ptr<nano::block> const & block)
{
	auto future = blocking.insert (block);
	add_impl (block);
	rsnano::rsn_block_processor_notify_all (handle);
	std::optional<nano::process_return> result;
	try
	{
		auto status = future.wait_for (config.block_process_timeout);
		debug_assert (status != std::future_status::deferred);
		if (status == std::future_status::ready)
		{
			result = future.get ();
		}
		else
		{
			blocking.erase (block);
		}
	}
	catch (std::future_error const &)
	{
	}
	return result;
}

void nano::block_processor::rollback_competitor (nano::write_transaction const & transaction, nano::block const & block)
{
	auto hash = block.hash ();
	auto successor = ledger.successor (transaction, block.qualified_root ());
	if (successor != nullptr && successor->hash () != hash)
	{
		// Replace our block with the winner and roll back any dependent blocks
		if (config.logging.ledger_rollback_logging ())
		{
			logger.always_log (boost::str (boost::format ("Rolling back %1% and replacing with %2%") % successor->hash ().to_string () % hash.to_string ()));
		}
		std::vector<std::shared_ptr<nano::block>> rollback_list;
		if (ledger.rollback (transaction, successor->hash (), rollback_list))
		{
			stats.inc (nano::stat::type::ledger, nano::stat::detail::rollback_failed);
			logger.always_log (nano::severity_level::error, boost::str (boost::format ("Failed to roll back %1% because it or a successor was confirmed") % successor->hash ().to_string ()));
		}
		else if (config.logging.ledger_rollback_logging ())
		{
			logger.always_log (boost::str (boost::format ("%1% blocks rolled back") % rollback_list.size ()));
		}
		blocks_rolled_back (rollback_list, successor);
	}
}

void nano::block_processor::force (std::shared_ptr<nano::block> const & block_a)
{
	{
		nano::block_processor_lock lock{ handle };
		forced.push_back (block_a);
	}
	rsnano::rsn_block_processor_notify_all (handle);
}

void nano::block_processor::process_blocks ()
{
	nano::block_processor_lock lock{ handle };
	while (!stopped)
	{
		if (have_blocks_ready ())
		{
			active = true;
			lock.unlock ();
			auto processed = process_batch (lock);
			batch_processed.notify (processed);
			lock.lock (handle);
			active = false;
		}
		else
		{
			rsnano::rsn_block_processor_notify_one (handle);
			rsnano::rsn_block_processor_wait (handle, lock.handle);
		}
	}
}

bool nano::block_processor::should_log ()
{
	auto result (false);
	auto now (std::chrono::steady_clock::now ());
	if (next_log < now)
	{
		next_log = now + (config.logging.timing_logging () ? std::chrono::seconds (2) : std::chrono::seconds (15));
		result = true;
	}
	return result;
}

bool nano::block_processor::have_blocks_ready ()
{
	return !blocks.empty () || !forced.empty ();
}

bool nano::block_processor::have_blocks ()
{
	return have_blocks_ready () || state_block_signature_verification.size () != 0;
}

void nano::block_processor::process_verified_state_blocks (std::deque<nano::state_block_signature_verification::value_type> & items, std::vector<int> const & verifications, std::vector<nano::block_hash> const & hashes, std::vector<nano::signature> const & blocks_signatures)
{
	{
		nano::block_processor_lock lk{ handle };
		for (auto i (0); i < verifications.size (); ++i)
		{
			debug_assert (verifications[i] == 1 || verifications[i] == 0);
			auto & item = items.front ();
			auto & [block] = item;
			if (!block->link ().is_zero () && ledger.is_epoch_link (block->link ()))
			{
				// Epoch blocks
				if (verifications[i] == 1)
				{
					blocks.emplace_back (block);
				}
				else
				{
					// Possible regular state blocks with epoch link (send subtype)
					blocks.emplace_back (block);
				}
			}
			else if (verifications[i] == 1)
			{
				// Non epoch blocks
				blocks.emplace_back (block);
			}
			items.pop_front ();
		}
	}
	rsnano::rsn_block_processor_notify_all (handle);
}

void nano::block_processor::add_impl (std::shared_ptr<nano::block> block)
{
	if (block->type () == nano::block_type::state || block->type () == nano::block_type::open)
	{
		state_block_signature_verification.add ({ block });
	}
	else
	{
		{
			block_processor_lock lock{ handle };
			blocks.emplace_back (block);
		}
		rsnano::rsn_block_processor_notify_all (handle);
	}
}

auto nano::block_processor::process_batch (nano::block_processor_lock & lock_a) -> std::deque<processed_t>
{
	std::deque<processed_t> processed;
	auto scoped_write_guard = write_database_queue.wait (nano::writer::process_batch);
	auto transaction (store.tx_begin_write ({ tables::accounts, tables::blocks, tables::frontiers, tables::pending }));
	nano::timer<std::chrono::milliseconds> timer_l;
	lock_a.lock (handle);
	timer_l.start ();
	// Processing blocks
	unsigned number_of_blocks_processed (0), number_of_forced_processed (0);
	auto deadline_reached = [&timer_l, deadline = config.block_processor_batch_max_time] { return timer_l.after_deadline (deadline); };
	auto processor_batch_reached = [&number_of_blocks_processed, max = flags.block_processor_batch_size ()] { return number_of_blocks_processed >= max; };
	auto store_batch_reached = [&number_of_blocks_processed, max = store.max_block_write_batch_num ()] { return number_of_blocks_processed >= max; };
	while (have_blocks_ready () && (!deadline_reached () || !processor_batch_reached ()) && !store_batch_reached ())
	{
		if ((blocks.size () + state_block_signature_verification.size () + forced.size () > 64) && should_log ())
		{
			logger.always_log (boost::str (boost::format ("%1% blocks (+ %2% state blocks) (+ %3% forced) in processing queue") % blocks.size () % state_block_signature_verification.size () % forced.size ()));
		}
		std::shared_ptr<nano::block> block;
		nano::block_hash hash (0);
		bool force (false);
		if (forced.empty ())
		{
			block = blocks.front ();
			blocks.pop_front ();
			hash = block->hash ();
		}
		else
		{
			block = forced.front ();
			forced.pop_front ();
			hash = block->hash ();
			force = true;
			number_of_forced_processed++;
		}
		lock_a.unlock ();
		if (force)
		{
			rollback_competitor (*transaction, *block);
		}
		number_of_blocks_processed++;
		auto result = process_one (*transaction, block, force);
		processed.emplace_back (result, block);
		lock_a.lock (handle);
	}
	lock_a.unlock ();

	if (config.logging.timing_logging () && number_of_blocks_processed != 0 && timer_l.stop () > std::chrono::milliseconds (100))
	{
		logger.always_log (boost::str (boost::format ("Processed %1% blocks (%2% blocks were forced) in %3% %4%") % number_of_blocks_processed % number_of_forced_processed % timer_l.value ().count () % timer_l.unit ()));
	}
	return processed;
}

nano::process_return nano::block_processor::process_one (nano::write_transaction const & transaction_a, std::shared_ptr<nano::block> block, bool const forced_a)
{
	nano::process_return result;
	auto hash (block->hash ());
	result = ledger.process (transaction_a, *block);
	switch (result.code)
	{
		case nano::process_result::progress:
		{
			if (config.logging.ledger_logging ())
			{
				std::string block_string;
				block->serialize_json (block_string, config.logging.single_line_record ());
				logger.try_log (boost::str (boost::format ("Processing block %1%: %2%") % hash.to_string () % block_string));
			}
			queue_unchecked (transaction_a, hash);
			/* For send blocks check epoch open unchecked (gap pending).
			For state blocks check only send subtype and only if block epoch is not last epoch.
			If epoch is last, then pending entry shouldn't trigger same epoch open block for destination account. */
			if (block->type () == nano::block_type::send || (block->type () == nano::block_type::state && block->sideband ().details ().is_send () && std::underlying_type_t<nano::epoch> (block->sideband ().details ().epoch ()) < std::underlying_type_t<nano::epoch> (nano::epoch::max)))
			{
				/* block->destination () for legacy send blocks
				block->link () for state blocks (send subtype) */
				queue_unchecked (transaction_a, block->destination ().is_zero () ? block->link () : block->destination ());
			}
			break;
		}
		case nano::process_result::gap_previous:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Gap previous for: %1%") % hash.to_string ()));
			}
			unchecked.put (block->previous (), block);
			stats.inc (nano::stat::type::ledger, nano::stat::detail::gap_previous);
			break;
		}
		case nano::process_result::gap_source:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Gap source for: %1%") % hash.to_string ()));
			}
			unchecked.put (ledger.block_source (transaction_a, *block), block);
			stats.inc (nano::stat::type::ledger, nano::stat::detail::gap_source);
			break;
		}
		case nano::process_result::gap_epoch_open_pending:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Gap pending entries for epoch open: %1%") % hash.to_string ()));
			}
			unchecked.put (block->account (), block); // Specific unchecked key starting with epoch open block account public key
			stats.inc (nano::stat::type::ledger, nano::stat::detail::gap_source);
			break;
		}
		case nano::process_result::old:
		{
			if (config.logging.ledger_duplicate_logging ())
			{
				logger.try_log (boost::str (boost::format ("Old for: %1%") % hash.to_string ()));
			}
			stats.inc (nano::stat::type::ledger, nano::stat::detail::old);
			break;
		}
		case nano::process_result::bad_signature:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Bad signature for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::negative_spend:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Negative spend for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::unreceivable:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Unreceivable for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::fork:
		{
			stats.inc (nano::stat::type::ledger, nano::stat::detail::fork);
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Fork for: %1% root: %2%") % hash.to_string () % block->root ().to_string ()));
			}
			break;
		}
		case nano::process_result::opened_burn_account:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Rejecting open block for burn account: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::balance_mismatch:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Balance mismatch for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::representative_mismatch:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Representative mismatch for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::block_position:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Block %1% cannot follow predecessor %2%") % hash.to_string () % block->previous ().to_string ()));
			}
			break;
		}
		case nano::process_result::insufficient_work:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Insufficient work for %1% : %2% (difficulty %3%)") % hash.to_string () % nano::to_string_hex (block->block_work ()) % nano::to_string_hex (network_params.work.difficulty (*block))));
			}
			break;
		}
	}

	stats.inc (nano::stat::type::blockprocessor, nano::to_stat_detail (result.code));

	return result;
}

void nano::block_processor::queue_unchecked (nano::write_transaction const & transaction_a, nano::hash_or_account const & hash_or_account_a)
{
	unchecked.trigger (hash_or_account_a);
	gap_cache.erase (hash_or_account_a.hash);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (block_processor & block_processor, std::string const & name)
{
	std::size_t blocks_count;
	std::size_t forced_count;

	{
		nano::block_processor_lock lock{ block_processor.handle };
		blocks_count = block_processor.blocks.size ();
		forced_count = block_processor.forced.size ();
	}

	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (collect_container_info (block_processor.state_block_signature_verification, "state_block_signature_verification"));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "blocks", blocks_count, sizeof (decltype (block_processor.blocks)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "forced", forced_count, sizeof (decltype (block_processor.forced)::value_type) }));
	return composite;
}
