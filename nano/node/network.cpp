#include "nano/lib/rsnano.hpp"

#include <nano/crypto_lib/random_pool_shuffle.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/bootstrap_ascending/service.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>
#include <nano/node/telemetry.hpp>

#include <boost/format.hpp>

#include <exception>

/*
 * network
 */

nano::network::network (nano::node & node_a, uint16_t port_a) :
	id (nano::network_constants::active_network ()),
	syn_cookies{ std::make_shared<nano::syn_cookies> (node_a.network_params.network.max_peers_per_ip) },
	inbound{ [this] (nano::message const & message, std::shared_ptr<nano::transport::channel> const & channel) {
		debug_assert (message.get_header ().get_network () == node.network_params.network.current_network);
		debug_assert (message.get_header ().get_version_using () >= node.network_params.network.protocol_version_min);
		process_message (message, channel);
	} },
	resolver (node_a.io_ctx),
	node (node_a),
	port (port_a),
	disconnect_observer ([] () {})
{
}

nano::network::~network ()
{
	stop ();
}

void nano::network::start_threads ()
{
	tcp_channels = std::move (std::make_shared<nano::transport::tcp_channels> (node, port, inbound));
	auto this_l = shared_from_this ();
	// TCP
	for (std::size_t i = 0; i < node.config->network_threads && !node.flags.disable_tcp_realtime (); ++i)
	{
		auto this_l = shared_from_this ();
		packet_processing_threads.emplace_back (nano::thread_attributes::get_default (), [this_l] () {
			nano::thread_role::set (nano::thread_role::name::packet_processing);
			try
			{
				this_l->tcp_channels->process_messages ();
			}
			catch (boost::system::error_code & ec)
			{
				this_l->node.logger->always_log (FATAL_LOG_PREFIX, ec.message ());
				release_assert (false);
			}
			catch (std::error_code & ec)
			{
				this_l->node.logger->always_log (FATAL_LOG_PREFIX, ec.message ());
				release_assert (false);
			}
			catch (std::runtime_error & err)
			{
				this_l->node.logger->always_log (FATAL_LOG_PREFIX, err.what ());
				release_assert (false);
			}
			catch (std::exception & err)
			{
				std::cerr << err.what () << std::endl;
				this_l->node.logger->always_log (FATAL_LOG_PREFIX, err.what ());
				release_assert (false);
			}
			catch (...)
			{
				this_l->node.logger->always_log (FATAL_LOG_PREFIX, "Unknown exception");
				release_assert (false);
			}
			if (this_l->node.config->logging.network_packet_logging ())
			{
				this_l->node.logger->try_log ("Exiting TCP packet processing thread");
			}
		});
	}
}

void nano::network::start ()
{
	if (!node.flags.disable_connection_cleanup ())
	{
		ongoing_cleanup ();
	}
	ongoing_syn_cookie_cleanup ();
	if (!node.flags.disable_tcp_realtime ())
	{
		tcp_channels->start ();
	}
	ongoing_keepalive ();
}

void nano::network::stop ()
{
	if (!stopped.exchange (true))
	{
		if (tcp_channels)
			tcp_channels->stop ();
		resolver.cancel ();
		port = 0;
		for (auto & thread : packet_processing_threads)
		{
			thread.join ();
		}
	}
}

void nano::network::send_keepalive (std::shared_ptr<nano::transport::channel> const & channel_a)
{
	nano::keepalive message{ node.network_params.network };
	std::array<nano::endpoint, 8> peers;
	tcp_channels->random_fill (peers);
	message.set_peers (peers);
	channel_a->send (message);
}

void nano::network::send_keepalive_self (std::shared_ptr<nano::transport::channel> const & channel_a)
{
	nano::keepalive message{ node.network_params.network };
	auto peers{ message.get_peers () };
	fill_keepalive_self (peers);
	message.set_peers (peers);
	channel_a->send (message);
}

void nano::network::send_node_id_handshake (std::shared_ptr<nano::transport::channel> const & channel_a, std::optional<nano::uint256_union> const & cookie, std::optional<nano::uint256_union> const & respond_to)
{
	std::optional<nano::node_id_handshake::response_payload> response;
	if (respond_to)
	{
		nano::node_id_handshake::response_payload pld{ node.node_id.pub, nano::sign_message (node.node_id.prv, node.node_id.pub, *respond_to) };
		debug_assert (!nano::validate_message (pld.node_id, *respond_to, pld.signature));
		response = pld;
	}

	std::optional<nano::node_id_handshake::query_payload> query;
	if (cookie)
	{
		nano::node_id_handshake::query_payload pld{ *cookie };
		query = pld;
	}

	nano::node_id_handshake message{ node.network_params.network, query, response };
	if (node.config->logging.network_node_id_handshake_logging ())
	{
		node.logger->try_log (boost::str (boost::format ("Node ID handshake sent with node ID %1% to %2%: query %3%, respond_to %4% (signature %5%)") % node.node_id.pub.to_node_id () % channel_a->get_remote_endpoint () % (query ? query->cookie.to_string () : std::string ("[none]")) % (respond_to ? respond_to->to_string () : std::string ("[none]")) % (response ? response->signature.to_string () : std::string ("[none]"))));
	}

	channel_a->send (message);
}

void nano::network::flood_message (nano::message & message_a, nano::transport::buffer_drop_policy const drop_policy_a, float const scale_a)
{
	for (auto & i : list (fanout (scale_a)))
	{
		i->send (message_a, nullptr, drop_policy_a);
	}
}

void nano::network::flood_keepalive (float const scale_a)
{
	nano::keepalive message{ node.network_params.network };
	auto peers{ message.get_peers () };
	tcp_channels->random_fill (peers);
	message.set_peers (peers);
	flood_message (message, nano::transport::buffer_drop_policy::limiter, scale_a);
}

void nano::network::flood_keepalive_self (float const scale_a)
{
	nano::keepalive message{ node.network_params.network };
	auto peers{ message.get_peers () };
	fill_keepalive_self (peers);
	message.set_peers (peers);
	flood_message (message, nano::transport::buffer_drop_policy::limiter, scale_a);
}

void nano::network::flood_block (std::shared_ptr<nano::block> const & block_a, nano::transport::buffer_drop_policy const drop_policy_a)
{
	nano::publish message (node.network_params.network, block_a);
	flood_message (message, drop_policy_a);
}

void nano::network::flood_block_initial (std::shared_ptr<nano::block> const & block_a)
{
	nano::publish message (node.network_params.network, block_a);
	for (auto const & i : node.rep_crawler.principal_representatives ())
	{
		i.get_channel ()->send (message, nullptr, nano::transport::buffer_drop_policy::no_limiter_drop);
	}
	for (auto & i : list_non_pr (fanout (1.0)))
	{
		i->send (message, nullptr, nano::transport::buffer_drop_policy::no_limiter_drop);
	}
}

void nano::network::flood_vote (std::shared_ptr<nano::vote> const & vote_a, float scale)
{
	nano::confirm_ack message{ node.network_params.network, vote_a };
	for (auto & i : list (fanout (scale)))
	{
		i->send (message, nullptr);
	}
}

void nano::network::flood_vote_pr (std::shared_ptr<nano::vote> const & vote_a)
{
	nano::confirm_ack message{ node.network_params.network, vote_a };
	for (auto const & i : node.rep_crawler.principal_representatives ())
	{
		i.get_channel ()->send (message, nullptr, nano::transport::buffer_drop_policy::no_limiter_drop);
	}
}

void nano::network::flood_block_many (std::deque<std::shared_ptr<nano::block>> blocks_a, std::function<void ()> callback_a, unsigned delay_a)
{
	if (!blocks_a.empty ())
	{
		auto block_l (blocks_a.front ());
		blocks_a.pop_front ();
		flood_block (block_l);
		if (!blocks_a.empty ())
		{
			std::weak_ptr<nano::node> node_w (node.shared ());
			node.workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::milliseconds (delay_a + std::rand () % delay_a), [node_w, blocks (std::move (blocks_a)), callback_a, delay_a] () {
				if (auto node_l = node_w.lock ())
				{
					node_l->network->flood_block_many (std::move (blocks), callback_a, delay_a);
				}
			});
		}
		else if (callback_a)
		{
			callback_a ();
		}
	}
}

void nano::network::send_confirm_req (std::shared_ptr<nano::transport::channel> const & channel_a, std::pair<nano::block_hash, nano::block_hash> const & hash_root_a)
{
	// Confirmation request with hash + root
	nano::confirm_req req (node.network_params.network, hash_root_a.first, hash_root_a.second);
	channel_a->send (req);
}

void nano::network::broadcast_confirm_req_base (std::shared_ptr<nano::block> const & block_a, std::shared_ptr<std::vector<std::shared_ptr<nano::transport::channel>>> const & endpoints_a, unsigned delay_a, bool resumption)
{
	std::size_t const max_reps = 10;
	if (!resumption && node.config->logging.network_logging ())
	{
		node.logger->try_log (boost::str (boost::format ("Broadcasting confirm req for block %1% to %2% representatives") % block_a->hash ().to_string () % endpoints_a->size ()));
	}
	auto count (0);
	while (!endpoints_a->empty () && count < max_reps)
	{
		auto channel (endpoints_a->back ());
		send_confirm_req (channel, std::make_pair (block_a->hash (), block_a->root ().as_block_hash ()));
		endpoints_a->pop_back ();
		count++;
	}
	if (!endpoints_a->empty ())
	{
		delay_a += std::rand () % broadcast_interval_ms;

		std::weak_ptr<nano::node> node_w (node.shared ());
		node.workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::milliseconds (delay_a), [node_w, block_a, endpoints_a, delay_a] () {
			if (auto node_l = node_w.lock ())
			{
				node_l->network->broadcast_confirm_req_base (block_a, endpoints_a, delay_a, true);
			}
		});
	}
}

void nano::network::broadcast_confirm_req_batched_many (std::unordered_map<std::shared_ptr<nano::transport::channel>, std::deque<std::pair<nano::block_hash, nano::root>>> request_bundle_a, std::function<void ()> callback_a, unsigned delay_a, bool resumption_a)
{
	if (!resumption_a && node.config->logging.network_logging ())
	{
		node.logger->try_log (boost::str (boost::format ("Broadcasting batch confirm req to %1% representatives") % request_bundle_a.size ()));
	}

	for (auto i (request_bundle_a.begin ()), n (request_bundle_a.end ()); i != n;)
	{
		std::vector<std::pair<nano::block_hash, nano::root>> roots_hashes_l;
		// Limit max request size hash + root to 7 pairs
		while (roots_hashes_l.size () < confirm_req_hashes_max && !i->second.empty ())
		{
			// expects ordering by priority, descending
			roots_hashes_l.push_back (i->second.front ());
			i->second.pop_front ();
		}
		nano::confirm_req req{ node.network_params.network, roots_hashes_l };
		i->first->send (req);
		if (i->second.empty ())
		{
			i = request_bundle_a.erase (i);
		}
		else
		{
			++i;
		}
	}
	if (!request_bundle_a.empty ())
	{
		std::weak_ptr<nano::node> node_w (node.shared ());
		node.workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::milliseconds (delay_a), [node_w, request_bundle_a, callback_a, delay_a] () {
			if (auto node_l = node_w.lock ())
			{
				node_l->network->broadcast_confirm_req_batched_many (request_bundle_a, callback_a, delay_a, true);
			}
		});
	}
	else if (callback_a)
	{
		callback_a ();
	}
}

void nano::network::broadcast_confirm_req_many (std::deque<std::pair<std::shared_ptr<nano::block>, std::shared_ptr<std::vector<std::shared_ptr<nano::transport::channel>>>>> requests_a, std::function<void ()> callback_a, unsigned delay_a)
{
	auto pair_l (requests_a.front ());
	requests_a.pop_front ();
	auto block_l (pair_l.first);
	// confirm_req to representatives
	auto endpoints (pair_l.second);
	if (!endpoints->empty ())
	{
		broadcast_confirm_req_base (block_l, endpoints, delay_a);
	}
	/* Continue while blocks remain
	Broadcast with random delay between delay_a & 2*delay_a */
	if (!requests_a.empty ())
	{
		std::weak_ptr<nano::node> node_w (node.shared ());
		node.workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::milliseconds (delay_a + std::rand () % delay_a), [node_w, requests_a, callback_a, delay_a] () {
			if (auto node_l = node_w.lock ())
			{
				node_l->network->broadcast_confirm_req_many (requests_a, callback_a, delay_a);
			}
		});
	}
	else if (callback_a)
	{
		callback_a ();
	}
}

namespace
{
class network_message_visitor : public nano::message_visitor
{
public:
	network_message_visitor (nano::node & node_a, std::shared_ptr<nano::transport::channel> const & channel_a) :
		node (node_a),
		channel (channel_a)
	{
	}

	void keepalive (nano::keepalive const & message_a) override
	{
		if (node.config->logging.network_keepalive_logging ())
		{
			node.logger->try_log (boost::str (boost::format ("Received keepalive message from %1%") % channel->to_string ()));
		}

		node.network->merge_peers (message_a.get_peers ());

		// Check for special node port data
		auto peer0 (message_a.get_peers ()[0]);
		if (peer0.address () == boost::asio::ip::address_v6{} && peer0.port () != 0)
		{
			nano::endpoint new_endpoint (channel->get_tcp_remote_endpoint ().address (), peer0.port ());
			node.network->merge_peer (new_endpoint);

			// Remember this for future forwarding to other peers
			channel->set_peering_endpoint (new_endpoint);
		}
	}

	void publish (nano::publish const & message_a) override
	{
		if (node.config->logging.network_message_logging ())
		{
			node.logger->try_log (boost::str (boost::format ("Publish message from %1% for %2%") % channel->to_string () % message_a.get_block ()->hash ().to_string ()));
		}

		if (!node.block_processor.full ())
		{
			auto block{ message_a.get_block () };
			node.process_active (block);
		}
		else
		{
			node.network->clear_from_publish_filter (message_a.get_digest ());
			node.stats->inc (nano::stat::type::drop, nano::stat::detail::publish, nano::stat::dir::in);
		}
	}

	void confirm_req (nano::confirm_req const & message_a) override
	{
		if (node.config->logging.network_message_logging ())
		{
			if (!message_a.get_roots_hashes ().empty ())
			{
				node.logger->try_log (boost::str (boost::format ("Confirm_req message from %1% for hashes:roots %2%") % channel->to_string () % message_a.roots_string ()));
			}
			else
			{
				node.logger->try_log (boost::str (boost::format ("Confirm_req message from %1% for %2%") % channel->to_string () % message_a.get_block ()->hash ().to_string ()));
			}
		}

		// Don't load nodes with disabled voting
		if (node.config->enable_voting && node.wallets.reps ().voting > 0)
		{
			if (message_a.get_block () != nullptr)
			{
				node.aggregator.add (channel, { { message_a.get_block ()->hash (), message_a.get_block ()->root () } });
			}
			else if (!message_a.get_roots_hashes ().empty ())
			{
				node.aggregator.add (channel, message_a.get_roots_hashes ());
			}
		}
	}

	void confirm_ack (nano::confirm_ack const & message_a) override
	{
		if (node.config->logging.network_message_logging ())
		{
			node.logger->try_log (boost::str (boost::format ("Received confirm_ack message from %1% for %2% timestamp %3%") % channel->to_string () % message_a.get_vote ()->hashes_string () % std::to_string (message_a.get_vote ()->timestamp ())));
		}

		if (!message_a.get_vote ()->account ().is_zero ())
		{
			node.vote_processor.vote (message_a.get_vote (), channel);
		}
	}

	void bulk_pull (nano::bulk_pull const &) override
	{
		debug_assert (false);
	}

	void bulk_pull_account (nano::bulk_pull_account const &) override
	{
		debug_assert (false);
	}

	void bulk_push (nano::bulk_push const &) override
	{
		debug_assert (false);
	}

	void frontier_req (nano::frontier_req const &) override
	{
		debug_assert (false);
	}

	void node_id_handshake (nano::node_id_handshake const & message_a) override
	{
		node.stats->inc (nano::stat::type::message, nano::stat::detail::node_id_handshake, nano::stat::dir::in);
	}

	void telemetry_req (nano::telemetry_req const & message_a) override
	{
		if (node.config->logging.network_telemetry_logging ())
		{
			node.logger->try_log (boost::str (boost::format ("Telemetry_req message from %1%") % channel->to_string ()));
		}

		// Send an empty telemetry_ack if we do not want, just to acknowledge that we have received the message to
		// remove any timeouts on the server side waiting for a message.
		nano::telemetry_ack telemetry_ack{ node.network_params.network };
		if (!node.flags.disable_providing_telemetry_metrics ())
		{
			auto telemetry_data = node.local_telemetry ();
			telemetry_ack = nano::telemetry_ack{ node.network_params.network, telemetry_data };
		}
		channel->send (telemetry_ack, nullptr, nano::transport::buffer_drop_policy::no_socket_drop);
	}

	void telemetry_ack (nano::telemetry_ack const & message_a) override
	{
		if (node.config->logging.network_telemetry_logging ())
		{
			node.logger->try_log (boost::str (boost::format ("Received telemetry_ack message from %1%") % channel->to_string ()));
		}

		node.telemetry->process (message_a, channel);
	}

	void asc_pull_req (nano::asc_pull_req const & message) override
	{
		node.bootstrap_server.request (message, channel);
	}

	void asc_pull_ack (nano::asc_pull_ack const & message) override
	{
		node.ascendboot.process (message, channel);
	}

private:
	nano::node & node;
	std::shared_ptr<nano::transport::channel> channel;
};
}

void nano::network::process_message (nano::message const & message, std::shared_ptr<nano::transport::channel> const & channel)
{
	node.stats->inc (nano::stat::type::message, nano::to_stat_detail (message.type ()), nano::stat::dir::in);

	network_message_visitor visitor (node, channel);
	message.visit (visitor);
}

// Send keepalives to all the peers we've been notified of
void nano::network::merge_peers (std::array<nano::endpoint, 8> const & peers_a)
{
	for (auto i (peers_a.begin ()), j (peers_a.end ()); i != j; ++i)
	{
		merge_peer (*i);
	}
}

void nano::network::merge_peer (nano::endpoint const & peer_a)
{
	if (!reachout (peer_a, node.config->allow_local_peers))
	{
		std::weak_ptr<nano::node> node_w (node.shared ());
		node.network->tcp_channels->start_tcp (peer_a);
	}
}

bool nano::network::reachout (nano::endpoint const & endpoint_a, bool allow_local_peers)
{
	// Don't contact invalid IPs
	bool error = tcp_channels->not_a_peer (endpoint_a, allow_local_peers);
	if (!error)
	{
		error = tcp_channels->reachout (endpoint_a);
	}
	return error;
}

std::deque<std::shared_ptr<nano::transport::channel>> nano::network::list (std::size_t count_a, uint8_t minimum_version_a, bool include_tcp_temporary_channels_a)
{
	std::deque<std::shared_ptr<nano::transport::channel>> result;
	tcp_channels->list (result, minimum_version_a, include_tcp_temporary_channels_a);
	nano::random_pool_shuffle (result.begin (), result.end ());
	if (count_a > 0 && result.size () > count_a)
	{
		result.resize (count_a, nullptr);
	}
	return result;
}

std::deque<std::shared_ptr<nano::transport::channel>> nano::network::list_non_pr (std::size_t count_a)
{
	std::deque<std::shared_ptr<nano::transport::channel>> result;
	tcp_channels->list (result);
	nano::random_pool_shuffle (result.begin (), result.end ());
	result.erase (std::remove_if (result.begin (), result.end (), [this] (std::shared_ptr<nano::transport::channel> const & channel) {
		return this->node.rep_crawler.is_pr (*channel);
	}),
	result.end ());
	if (result.size () > count_a)
	{
		result.resize (count_a, nullptr);
	}
	return result;
}

// Simulating with sqrt_broadcast_simulate shows we only need to broadcast to sqrt(total_peers) random peers in order to successfully publish to everyone with high probability
std::size_t nano::network::fanout (float scale) const
{
	return static_cast<std::size_t> (std::ceil (scale * size_sqrt ()));
}

std::vector<std::shared_ptr<nano::transport::channel>> nano::network::random_channels (std::size_t count_a, uint8_t min_version_a, bool include_temporary_channels_a) const
{
	return tcp_channels->random_channels (count_a, min_version_a, include_temporary_channels_a);
}

void nano::network::fill_keepalive_self (std::array<nano::endpoint, 8> & target_a) const
{
	tcp_channels->random_fill (target_a);
	// We will clobber values in index 0 and 1 and if there are only 2 nodes in the system, these are the only positions occupied
	// Move these items to index 2 and 3 so they propagate
	target_a[2] = target_a[0];
	target_a[3] = target_a[1];
	// Replace part of message with node external address or listening port
	target_a[1] = nano::endpoint (boost::asio::ip::address_v6{}, 0); // For node v19 (response channels)
	if (node.config->external_address != boost::asio::ip::address_v6{}.to_string () && node.config->external_port != 0)
	{
		target_a[0] = nano::endpoint (boost::asio::ip::make_address_v6 (node.config->external_address), node.config->external_port);
	}
	else
	{
		auto external_address (node.port_mapping.external_address ());
		if (external_address.address () != boost::asio::ip::address_v4::any ())
		{
			target_a[0] = nano::endpoint (boost::asio::ip::address_v6{}, port);
			boost::system::error_code ec;
			auto external_v6 = boost::asio::ip::make_address_v6 (external_address.address ().to_string (), ec);
			target_a[1] = nano::endpoint (external_v6, external_address.port ());
		}
		else
		{
			target_a[0] = nano::endpoint (boost::asio::ip::address_v6{}, port);
		}
	}
}

nano::tcp_endpoint nano::network::bootstrap_peer ()
{
	return tcp_channels->bootstrap_peer ();
}

std::shared_ptr<nano::transport::channel> nano::network::find_channel (nano::endpoint const & endpoint_a)
{
	return tcp_channels->find_channel (nano::transport::map_endpoint_to_tcp (endpoint_a));
}

std::shared_ptr<nano::transport::channel> nano::network::find_node_id (nano::account const & node_id_a)
{
	return tcp_channels->find_node_id (node_id_a);
}

nano::endpoint nano::network::endpoint () const
{
	return nano::endpoint (boost::asio::ip::address_v6::loopback (), port);
}

void nano::network::cleanup (std::chrono::system_clock::time_point const & cutoff_a)
{
	tcp_channels->purge (cutoff_a);
	if (node.network->empty ())
	{
		disconnect_observer ();
	}
}

void nano::network::ongoing_cleanup ()
{
	cleanup (std::chrono::system_clock::now () - node.network_params.network.cleanup_cutoff ());
	std::weak_ptr<nano::node> node_w (node.shared ());
	node.workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::seconds (node.network_params.network.is_dev_network () ? 1 : 5), [node_w] () {
		if (auto node_l = node_w.lock ())
		{
			node_l->network->ongoing_cleanup ();
		}
	});
}

void nano::network::ongoing_syn_cookie_cleanup ()
{
	syn_cookies->purge (nano::transport::syn_cookie_cutoff);
	std::weak_ptr<nano::node> node_w (node.shared ());
	node.workers->add_timed_task (std::chrono::steady_clock::now () + (nano::transport::syn_cookie_cutoff * 2), [node_w] () {
		if (auto node_l = node_w.lock ())
		{
			node_l->network->ongoing_syn_cookie_cleanup ();
		}
	});
}

void nano::network::ongoing_keepalive ()
{
	flood_keepalive (0.75f);
	flood_keepalive_self (0.25f);
	std::weak_ptr<nano::node> node_w (node.shared ());
	node.workers->add_timed_task (std::chrono::steady_clock::now () + node.network_params.network.keepalive_period, [node_w] () {
		if (auto node_l = node_w.lock ())
		{
			node_l->network->ongoing_keepalive ();
		}
	});
}

std::size_t nano::network::size () const
{
	return tcp_channels->size ();
}

float nano::network::size_sqrt () const
{
	return static_cast<float> (std::sqrt (size ()));
}

bool nano::network::empty () const
{
	return size () == 0;
}

void nano::network::erase (nano::transport::channel const & channel_a)
{
	auto const channel_type = channel_a.get_type ();
	if (channel_type == nano::transport::transport_type::tcp)
	{
		tcp_channels->erase (channel_a.get_tcp_remote_endpoint ());
	}
}

void nano::network::exclude (std::shared_ptr<nano::transport::channel> const & channel)
{
	// Add to peer exclusion list
	tcp_channels->excluded_peers ().add (channel->get_tcp_remote_endpoint ());

	// Disconnect
	erase (*channel);
}

/*
 * syn_cookies
 */

nano::syn_cookies::syn_cookies (std::size_t max_cookies_per_ip_a) :
	handle{ rsnano::rsn_syn_cookies_create (max_cookies_per_ip_a) }
{
}

nano::syn_cookies::~syn_cookies ()
{
	rsnano::rsn_syn_cookies_destroy (handle);
}

boost::optional<nano::uint256_union> nano::syn_cookies::assign (nano::endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	boost::optional<nano::uint256_union> result;
	nano::uint256_union cookie;
	if (rsnano::rsn_syn_cookies_assign (handle, &endpoint_dto, cookie.bytes.data ()))
		result = cookie;

	return result;
}

bool nano::syn_cookies::validate (nano::endpoint const & endpoint_a, nano::account const & node_id, nano::signature const & sig)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	bool ok = rsnano::rsn_syn_cookies_validate (handle, &endpoint_dto, node_id.bytes.data (), sig.bytes.data ());
	return !ok;
}

void nano::syn_cookies::purge (std::chrono::seconds const & cutoff_a)
{
	rsnano::rsn_syn_cookies_purge (handle, cutoff_a.count ());
}

std::optional<nano::uint256_union> nano::syn_cookies::cookie (const nano::endpoint & endpoint_a)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	nano::uint256_union cookie;
	if (rsnano::rsn_syn_cookies_cookie (handle, &endpoint_dto, cookie.bytes.data ()))
	{
		return cookie;
	}
	return std::nullopt;
}

std::size_t nano::syn_cookies::cookies_size ()
{
	return rsnano::rsn_syn_cookies_cookies_count (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (network & network, std::string const & name)
{
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (network.tcp_channels->collect_container_info ("tcp_channels"));
	composite->add_component (network.syn_cookies->collect_container_info ("syn_cookies"));
	composite->add_component (network.tcp_channels->excluded_peers ().collect_container_info ("excluded_peers"));
	return composite;
}

std::unique_ptr<nano::container_info_component> nano::syn_cookies::collect_container_info (std::string const & name)
{
	std::size_t syn_cookies_count = rsnano::rsn_syn_cookies_cookies_count (handle);
	std::size_t syn_cookies_per_ip_count = rsnano::rsn_syn_cookies_cookies_per_ip_count (handle);
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "syn_cookies", syn_cookies_count, rsnano::rsn_syn_cookies_cookie_info_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "syn_cookies_per_ip", syn_cookies_per_ip_count, rsnano::rsn_syn_cookies_cookies_per_ip_size () }));
	return composite;
}

std::string nano::network::to_string (nano::networks network)
{
	rsnano::StringDto result;
	rsnano::rsn_network_to_string (static_cast<uint16_t> (network), &result);
	return rsnano::convert_dto_to_string (result);
}

void nano::network::on_new_channel (std::function<void (std::shared_ptr<nano::transport::channel>)> observer_a)
{
	tcp_channels->on_new_channel (observer_a);
}

void nano::network::clear_from_publish_filter (nano::uint128_t const & digest_a)
{
	tcp_channels->publish_filter->clear (digest_a);
}

uint16_t nano::network::get_port ()
{
	return port;
}

void nano::network::set_port (uint16_t port_a)
{
	port = port_a;
	tcp_channels->set_port (port_a);
}
