#pragma once

#include <nano/lib/rpcconfig.hpp>

#include <boost/property_tree/ptree_fwd.hpp>

#include <string>

namespace boost
{
namespace filesystem
{
	class path;
}
}

namespace nano
{
class tomlconfig;
class rpc_child_process_config final
{
public:
	bool enable{ false };
	std::string rpc_path;
};

class node_rpc_config final
{
public:
	node_rpc_config ();
	void load_dto (rsnano::NodeRpcConfigDto & dto);
	rsnano::NodeRpcConfigDto to_dto () const;
	nano::error deserialize_toml (nano::tomlconfig & toml);

	bool enable_sign_hash{ false };
	nano::rpc_child_process_config child_process;

	// Used in tests to ensure requests are modified in specific cases
	void set_request_callback (std::function<void (boost::property_tree::ptree const &)>);
	std::function<void (boost::property_tree::ptree const &)> request_callback;
};
}
