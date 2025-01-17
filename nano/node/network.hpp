#pragma once

#include <nano/node/common.hpp>
#include <nano/node/peer_exclusion.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/secure/network_filter.hpp>

#include <boost/thread/thread.hpp>

#include <chrono>
#include <deque>
#include <memory>
#include <unordered_set>

namespace nano
{
class node;

/**
 * Node ID cookies for node ID handshakes
 */
class syn_cookies final
{
public:
	explicit syn_cookies (std::size_t);
	syn_cookies (nano::syn_cookies const &) = delete;
	~syn_cookies ();
	void purge (std::chrono::seconds const &);

	// Returns boost::none if the IP is rate capped on syn cookie requests,
	// or if the endpoint already has a syn cookie query
	boost::optional<nano::uint256_union> assign (nano::endpoint const &);
	// Returns false if valid, true if invalid (true on error convention)
	// Also removes the syn cookie from the store if valid
	bool validate (nano::endpoint const &, nano::account const &, nano::signature const &);
	/** Get cookie associated with endpoint and erases that cookie from this container */
	std::optional<nano::uint256_union> cookie (nano::endpoint const &);

	std::unique_ptr<container_info_component> collect_container_info (std::string const &);
	std::size_t cookies_size ();
	rsnano::SynCookiesHandle * handle;
};

class network final : public std::enable_shared_from_this<network>
{
public:
	network (nano::node &, uint16_t);
	~network ();

	nano::networks id;
	void start_threads ();
	void start ();
	void stop ();
	void flood_message (nano::message &, nano::transport::buffer_drop_policy const = nano::transport::buffer_drop_policy::limiter, float const = 1.0f);
	void flood_keepalive (float const scale_a = 1.0f);
	void flood_keepalive_self (float const scale_a = 0.5f);
	void flood_vote (std::shared_ptr<nano::vote> const &, float scale);
	void flood_vote_pr (std::shared_ptr<nano::vote> const &);
	// Flood block to all PRs and a random selection of non-PRs
	void flood_block_initial (std::shared_ptr<nano::block> const &);
	// Flood block to a random selection of peers
	void flood_block (std::shared_ptr<nano::block> const &, nano::transport::buffer_drop_policy const = nano::transport::buffer_drop_policy::limiter);
	void flood_block_many (std::deque<std::shared_ptr<nano::block>>, std::function<void ()> = nullptr, unsigned = broadcast_interval_ms);
	void merge_peers (std::array<nano::endpoint, 8> const &);
	void merge_peer (nano::endpoint const &);
	void send_keepalive (std::shared_ptr<nano::transport::channel> const &);
	void send_keepalive_self (std::shared_ptr<nano::transport::channel> const &);
	void send_node_id_handshake (std::shared_ptr<nano::transport::channel> const &, std::optional<nano::uint256_union> const & cookie, std::optional<nano::uint256_union> const & respond_to);
	void send_confirm_req (std::shared_ptr<nano::transport::channel> const & channel_a, std::pair<nano::block_hash, nano::block_hash> const & hash_root_a);
	void broadcast_confirm_req_base (std::shared_ptr<nano::block> const &, std::shared_ptr<std::vector<std::shared_ptr<nano::transport::channel>>> const &, unsigned, bool = false);
	void broadcast_confirm_req_batched_many (std::unordered_map<std::shared_ptr<nano::transport::channel>, std::deque<std::pair<nano::block_hash, nano::root>>>, std::function<void ()> = nullptr, unsigned = broadcast_interval_ms, bool = false);
	void broadcast_confirm_req_many (std::deque<std::pair<std::shared_ptr<nano::block>, std::shared_ptr<std::vector<std::shared_ptr<nano::transport::channel>>>>>, std::function<void ()> = nullptr, unsigned = broadcast_interval_ms);
	std::shared_ptr<nano::transport::channel> find_node_id (nano::account const &);
	std::shared_ptr<nano::transport::channel> find_channel (nano::endpoint const &);
	// Should we reach out to this endpoint with a keepalive message
	bool reachout (nano::endpoint const &, bool = false);
	std::deque<std::shared_ptr<nano::transport::channel>> list (std::size_t max_count = 0, uint8_t = 0, bool = true);
	std::deque<std::shared_ptr<nano::transport::channel>> list_non_pr (std::size_t);
	// Desired fanout for a given scale
	std::size_t fanout (float scale = 1.0f) const;
	void fill_keepalive_self (std::array<nano::endpoint, 8> &) const;
	// Note: The minimum protocol version is used after the random selection, so number of peers can be less than expected.
	std::vector<std::shared_ptr<nano::transport::channel>> random_channels (std::size_t count, uint8_t min_version = 0, bool include_temporary_channels = false) const;
	// Get the next peer for attempting a tcp bootstrap connection
	nano::tcp_endpoint bootstrap_peer ();
	nano::endpoint endpoint () const;
	void cleanup (std::chrono::system_clock::time_point const &);
	void ongoing_cleanup ();
	// Node ID cookies cleanup
	std::shared_ptr<nano::syn_cookies> syn_cookies;
	void ongoing_syn_cookie_cleanup ();
	void ongoing_keepalive ();
	std::size_t size () const;
	float size_sqrt () const;
	bool empty () const;
	void erase (nano::transport::channel const &);
	/** Disconnects and adds peer to exclusion list */
	void exclude (std::shared_ptr<nano::transport::channel> const & channel);

	static std::string to_string (nano::networks);
	void on_new_channel (std::function<void (std::shared_ptr<nano::transport::channel>)> observer_a);
	void clear_from_publish_filter (nano::uint128_t const & digest_a);
	uint16_t get_port ();
	void set_port (uint16_t port_a);

private:
	void process_message (nano::message const &, std::shared_ptr<nano::transport::channel> const &);

public:
	std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> inbound;
	boost::asio::ip::udp::resolver resolver;
	std::vector<boost::thread> packet_processing_threads;
	nano::node & node;
	std::shared_ptr<nano::transport::tcp_channels> tcp_channels;
	std::function<void ()> disconnect_observer;

public:
	std::atomic<uint16_t> port{ 0 };
	std::atomic<bool> stopped{ false };
	static unsigned const broadcast_interval_ms = 10;
	static std::size_t const buffer_size = 512;
	static std::size_t const confirm_req_hashes_max = 7;
	static std::size_t const confirm_ack_hashes_max = 12;
};
std::unique_ptr<container_info_component> collect_container_info (network & network, std::string const & name);
}
