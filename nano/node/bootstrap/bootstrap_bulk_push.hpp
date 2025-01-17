#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/node/common.hpp>

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
 * Client side of a bulk_push request. Sends a sequence of blocks the other side did not report in their frontier_req response.
 */
class bulk_push_client final : public std::enable_shared_from_this<nano::bulk_push_client>
{
public:
	explicit bulk_push_client (std::shared_ptr<nano::node> const &, std::shared_ptr<nano::bootstrap_client> const &, std::shared_ptr<nano::bootstrap_attempt_legacy> const &);
	~bulk_push_client ();
	void start ();
	void push ();
	void push_block (nano::block const &);
	void send_finished ();
	std::weak_ptr<nano::node> node_weak;
	std::shared_ptr<nano::bootstrap_client> connection;
	std::weak_ptr<nano::bootstrap_attempt_legacy> attempt;
	std::promise<bool> promise;
	std::pair<nano::block_hash, nano::block_hash> current_target;
};
}
