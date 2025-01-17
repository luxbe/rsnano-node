#pragma once

#include "nano/lib/config.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>

namespace nano
{
class node;

namespace transport
{
	/**
	 * In-process transport channel. Mostly useful for unit tests
	 **/
	namespace inproc
	{
		class channel final : public nano::transport::channel
		{
		public:
			explicit channel (nano::node & node, nano::node & destination);
			explicit channel (rsnano::ChannelHandle * handle_a);

			channel (
			size_t channel_id,
			nano::network_filter & publish_filter,
			nano::network_constants & network,
			nano::stats & stats,
			nano::outbound_bandwidth_limiter & outbound_limiter,
			boost::asio::io_context & io_ctx,
			nano::endpoint endpoint,
			nano::account source_node_id,
			std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> source_inbound,
			nano::endpoint destination,
			nano::account destination_node_id,
			std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> destination_inbound);

			uint8_t get_network_version () const override;

			std::size_t hash_code () const override;
			bool operator== (nano::transport::channel const &) const override;

			// TODO: investigate clang-tidy warning about default parameters on virtual/override functions
			//
			void send (nano::message & message_a,
			std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a = nullptr,
			nano::transport::buffer_drop_policy policy_a = nano::transport::buffer_drop_policy::limiter,
			nano::transport::traffic_type = nano::transport::traffic_type::generic) override;

			void send_buffer (nano::shared_const_buffer const &,
			std::function<void (boost::system::error_code const &, std::size_t)> const & = nullptr,
			nano::transport::buffer_drop_policy = nano::transport::buffer_drop_policy::limiter,
			nano::transport::traffic_type = nano::transport::traffic_type::generic) override;

			std::string to_string () const override;
			bool operator== (nano::transport::inproc::channel const & other_a) const
			{
				return get_remote_endpoint () == other_a.get_remote_endpoint ();
			}

			nano::tcp_endpoint get_local_endpoint () const override
			{
				return nano::tcp_endpoint{};
			}

			nano::endpoint get_remote_endpoint () const override;
			nano::tcp_endpoint get_tcp_remote_endpoint () const override;

			nano::transport::transport_type get_type () const override
			{
				return nano::transport::transport_type::loopback;
			}

			nano::endpoint get_peering_endpoint () const override;
			void set_peering_endpoint (nano::endpoint endpoint) override;
		};
	} // namespace inproc
} // namespace transport
} // namespace nano
