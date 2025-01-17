#pragma once

#include "nano/lib/numbers.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/node/common.hpp>

#include <deque>
#include <future>

namespace nano
{
class bootstrap_attempt_legacy;
class bootstrap_client;
class node;
namespace transport
{
	class tcp_server;
}

/**
 * Client side of a frontier request. Created to send and listen for frontier sequences from the server.
 */
class frontier_req_client final : public std::enable_shared_from_this<nano::frontier_req_client>
{
public:
	explicit frontier_req_client (std::shared_ptr<nano::node> const &, std::shared_ptr<nano::bootstrap_client> const &, std::shared_ptr<nano::bootstrap_attempt_legacy> const &);
	void run (nano::account const & start_account_a, uint32_t const frontiers_age_a, uint32_t const count_a);
	void receive_frontier ();
	void received_frontier (boost::system::error_code const &, std::size_t);
	bool bulk_push_available ();
	void unsynced (nano::block_hash const &, nano::block_hash const &);
	void next ();
	std::weak_ptr<nano::node> node_weak;
	std::shared_ptr<nano::bootstrap_client> connection;
	std::weak_ptr<nano::bootstrap_attempt_legacy> attempt;
	nano::account current;
	nano::block_hash frontier;
	unsigned count;
	nano::account last_account{ std::numeric_limits<nano::uint256_t>::max () }; // Using last possible account stop further frontier requests
	std::chrono::steady_clock::time_point start_time;
	std::promise<bool> promise;
	/** A very rough estimate of the cost of `bulk_push`ing missing blocks */
	uint64_t bulk_push_cost;
	std::deque<std::pair<nano::account, nano::block_hash>> accounts;
	uint32_t frontiers_age{ std::numeric_limits<uint32_t>::max () };
	uint32_t count_limit{ std::numeric_limits<uint32_t>::max () };
	static std::size_t constexpr size_frontier = sizeof (nano::account) + sizeof (nano::block_hash);
};

class frontier_req;

/**
 * Server side of a frontier request. Created when a tcp_server receives a frontier_req message and exited when end-of-list is reached.
 */
class frontier_req_server final : public std::enable_shared_from_this<nano::frontier_req_server>
{
public:
	frontier_req_server (std::shared_ptr<nano::node> const &, std::shared_ptr<nano::transport::tcp_server> const &, std::unique_ptr<nano::frontier_req>);
	frontier_req_server (frontier_req_server const &) = delete;
	~frontier_req_server ();
	void send_next ();
	nano::public_key current () const;
	nano::block_hash frontier () const;
	rsnano::FrontierReqServerHandle * handle;
};
}
