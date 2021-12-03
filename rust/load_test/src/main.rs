use anyhow::anyhow;
use anyhow::Result;
use clap::{App, Arg};
use rsnano::secure::DEV_NETWORK_PARAMS;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::time::Duration;

use rsnano::{
    config::{
        force_nano_dev_network, get_node_toml_config_path, get_rpc_toml_config_path, DaemonConfig,
        NetworkConstants, RpcConfig,
    },
    secure::{unique_path, NetworkParams},
    utils::TomlConfig,
};

const RPC_PORT_START: u16 = 60000;
const PEERING_PORT_START: u16 = 61000;
const IPC_PORT_START: u16 = 62000;

fn write_config_files(data_path: &Path, index: usize) -> Result<()> {
    let network_params = NetworkParams::new(NetworkConstants::active_network())?;
    let mut daemon_config = DaemonConfig::new(&network_params)?;
    daemon_config.node.peering_port = PEERING_PORT_START + index as u16;
    daemon_config
        .node
        .ipc_config
        .transport_tcp
        .transport
        .enabled = true;
    daemon_config.node.ipc_config.transport_tcp.port = IPC_PORT_START + index as u16;

    // Alternate use of memory pool
    daemon_config.node.use_memory_pools = (index % 2) == 0;

    // Write daemon config
    let mut toml = TomlConfig::new();
    daemon_config.serialize_toml(&mut toml)?;
    toml.write(get_node_toml_config_path(data_path))?;

    let mut rpc_config = RpcConfig::new(&network_params.network);
    rpc_config.port = RPC_PORT_START + index as u16;
    rpc_config.enable_control = true;
    rpc_config.rpc_process.ipc_port = IPC_PORT_START + index as u16;

    // Write rpc config
    let mut toml_rpc = TomlConfig::new();
    rpc_config.serialize_toml(&mut toml_rpc)?;
    toml_rpc.write(get_rpc_toml_config_path(data_path))?;
    Ok(())
}

// class account final
// {
// public:
// 	std::string private_key;
// 	std::string public_key;
// 	std::string as_string;
// };

// class account_info final
// {
// public:
// 	bool operator== (account_info const & other)
// 	{
// 		return frontier == other.frontier && block_count == other.block_count && balance == other.balance && error == other.error;
// 	}

// 	std::string frontier;
// 	std::string block_count;
// 	std::string balance;
// 	bool error{ false };
// };

// void send_receive (boost::asio::io_context & io_ctx, std::string const & wallet, std::string const & source, std::string const & destination, std::atomic<int> & send_calls_remaining, tcp::resolver::results_type const & results, boost::asio::yield_context yield)
// {
// 	boost::beast::flat_buffer buffer;
// 	http::request<http::string_body> req;
// 	http::response<http::string_body> res;
// 	socket_type socket (io_ctx);

// 	boost::asio::async_connect (socket, results.cbegin (), results.cend (), yield);

// 	boost::property_tree::ptree request;
// 	request.put ("action", "send");
// 	request.put ("wallet", wallet);
// 	request.put ("source", source);
// 	request.put ("destination", destination);
// 	request.put ("amount", "1");
// 	std::stringstream ostream;
// 	boost::property_tree::write_json (ostream, request);

// 	req.method (http::verb::post);
// 	req.version (11);
// 	req.target ("/");
// 	req.body () = ostream.str ();
// 	req.prepare_payload ();

// 	http::async_write (socket, req, yield);
// 	http::async_read (socket, buffer, res, yield);
// 	boost::property_tree::ptree json;
// 	std::stringstream body (res.body ());
// 	boost::property_tree::read_json (body, json);
// 	auto block = json.get<std::string> ("block");

// 	// Shut down send socket
// 	boost::system::error_code ec;
// 	socket.shutdown (tcp::socket::shutdown_both, ec);
// 	debug_assert (!ec || ec == boost::system::errc::not_connected);

// 	{
// 		// Start receive session
// 		boost::beast::flat_buffer buffer;
// 		http::request<http::string_body> req;
// 		http::response<http::string_body> res1;
// 		socket_type socket (io_ctx);

// 		boost::asio::async_connect (socket, results.cbegin (), results.cend (), yield);

// 		boost::property_tree::ptree request;
// 		request.put ("action", "receive");
// 		request.put ("wallet", wallet);
// 		request.put ("account", destination);
// 		request.put ("block", block);
// 		std::stringstream ostream;
// 		boost::property_tree::write_json (ostream, request);

// 		req.method (http::verb::post);
// 		req.version (11);
// 		req.target ("/");
// 		req.body () = ostream.str ();
// 		req.prepare_payload ();

// 		http::async_write (socket, req, yield);
// 		http::async_read (socket, buffer, res, yield);
// 		--send_calls_remaining;
// 		// Gracefully close the socket
// 		boost::system::error_code ec;
// 		socket.shutdown (tcp::socket::shutdown_both, ec);
// 		debug_assert (!ec || ec == boost::system::errc::not_connected);
// 	}
// }

// boost::property_tree::ptree rpc_request (boost::property_tree::ptree const & request, boost::asio::io_context & ioc, tcp::resolver::results_type const & results)
// {
// 	debug_assert (results.size () == 1);

// 	std::promise<boost::optional<boost::property_tree::ptree>> promise;
// 	boost::asio::spawn (ioc, [&ioc, &results, request, &promise] (boost::asio::yield_context yield) {
// 		socket_type socket (ioc);
// 		boost::beast::flat_buffer buffer;
// 		http::request<http::string_body> req;
// 		http::response<http::string_body> res;

// 		boost::asio::async_connect (socket, results.cbegin (), results.cend (), yield);
// 		std::stringstream ostream;
// 		boost::property_tree::write_json (ostream, request);

// 		req.method (http::verb::post);
// 		req.version (11);
// 		req.target ("/");
// 		req.body () = ostream.str ();
// 		req.prepare_payload ();

// 		http::async_write (socket, req, yield);
// 		http::async_read (socket, buffer, res, yield);

// 		boost::property_tree::ptree json;
// 		std::stringstream body (res.body ());
// 		boost::property_tree::read_json (body, json);
// 		promise.set_value (json);
// 	});

// 	auto future = promise.get_future ();
// 	if (future.wait_for (std::chrono::seconds (5)) != std::future_status::ready)
// 	{
// 		throw std::runtime_error ("RPC request timed out");
// 	}
// 	auto response = future.get ();
// 	debug_assert (response.is_initialized ());
// 	return response.value_or (decltype (response)::argument_type{});
// }

// void keepalive_rpc (boost::asio::io_context & ioc, tcp::resolver::results_type const & results, uint16_t port)
// {
// 	boost::property_tree::ptree request;
// 	request.put ("action", "keepalive");
// 	request.put ("address", "::1");
// 	request.put ("port", port);

// 	rpc_request (request, ioc, results);
// }

// account key_create_rpc (boost::asio::io_context & ioc, tcp::resolver::results_type const & results)
// {
// 	boost::property_tree::ptree request;
// 	request.put ("action", "key_create");

// 	auto json = rpc_request (request, ioc, results);

// 	account account_l;
// 	account_l.private_key = json.get<std::string> ("private");
// 	account_l.public_key = json.get<std::string> ("public");
// 	account_l.as_string = json.get<std::string> ("account");

// 	return account_l;
// }

// std::string wallet_create_rpc (boost::asio::io_context & ioc, tcp::resolver::results_type const & results)
// {
// 	boost::property_tree::ptree request;
// 	request.put ("action", "wallet_create");

// 	auto json = rpc_request (request, ioc, results);
// 	return json.get<std::string> ("wallet");
// }

// void wallet_add_rpc (boost::asio::io_context & ioc, tcp::resolver::results_type const & results, std::string const & wallet, std::string const & prv_key)
// {
// 	boost::property_tree::ptree request;
// 	request.put ("action", "wallet_add");
// 	request.put ("wallet", wallet);
// 	request.put ("key", prv_key);
// 	rpc_request (request, ioc, results);
// }

// void stop_rpc (boost::asio::io_context & ioc, tcp::resolver::results_type const & results)
// {
// 	boost::property_tree::ptree request;
// 	request.put ("action", "stop");
// 	rpc_request (request, ioc, results);
// }

// account_info account_info_rpc (boost::asio::io_context & ioc, tcp::resolver::results_type const & results, std::string const & account)
// {
// 	boost::property_tree::ptree request;
// 	request.put ("action", "account_info");
// 	request.put ("account", account);

// 	account_info account_info;
// 	auto json = rpc_request (request, ioc, results);

// 	auto error = json.get_optional<std::string> ("error");
// 	if (error)
// 	{
// 		account_info.error = true;
// 	}
// 	else
// 	{
// 		account_info.balance = json.get<std::string> ("balance");
// 		account_info.block_count = json.get<std::string> ("block_count");
// 		account_info.frontier = json.get<std::string> ("frontier");
// 	}
// 	return account_info;
// }

fn main() -> Result<()> {
    force_nano_dev_network();

    let matches = App::new("Nano Load Test")
        .about("This launches a node and fires a lot of send/recieve RPC requests at it (configurable), then other nodes are tested to make sure they observe these blocks as well.")
        .arg(Arg::with_name("node_count").short("n").long("node_count").help("The number of nodes to spin up").default_value("10"))
        .arg(Arg::with_name("node_path").long("node_path").takes_value(true).help( "The path to the nano_node to test"))
        .arg(Arg::with_name("rpc_path").long("rpc_path").takes_value(true).help("The path to the nano_rpc to test"))
        .get_matches();

    // 	boost::program_options::options_description description ("Command line options");

    // 	description.add_options ()
    // 		("send_count,s", boost::program_options::value<int> ()->default_value (2000), "How many send blocks to generate")
    // 		("simultaneous_process_calls", boost::program_options::value<int> ()->default_value (20), "Number of simultaneous rpc sends to do")
    // 		("destination_count", boost::program_options::value<int> ()->default_value (2), "How many destination accounts to choose between")

    let node_count = matches
        .value_of("node_count")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    // 	auto destination_count = vm.find ("destination_count")->second.as<int> ();
    // 	auto send_count = vm.find ("send_count")->second.as<int> ();
    // 	auto simultaneous_process_calls = vm.find ("simultaneous_process_calls")->second.as<int> ();

    // 	boost::system::error_code err;
    // 	auto running_executable_filepath = boost::dll::program_location (err);
    let running_executable_filepath = std::env::current_exe().unwrap();

    let node_path: PathBuf = match matches.value_of("node_path") {
        Some(p) => p.into(),
        None => {
            let mut node_filepath = running_executable_filepath.clone();
            node_filepath.pop(); //debug
            node_filepath.pop(); //build
            node_filepath.pop(); //cargo
            node_filepath.pop(); //root
            node_filepath.push("nano_node");
            if let Some(ext) = running_executable_filepath.extension() {
                node_filepath.set_extension(ext);
            }
            node_filepath
        }
    };

    if !node_path.exists() {
        panic!("nano_node executable could not be found in {:?}", node_path);
    }

    let rpc_path: PathBuf = match matches.value_of("rpc_path") {
        Some(p) => p.into(),
        None => {
            let mut rpc_filepath = running_executable_filepath.clone();
            rpc_filepath.pop(); //debug
            rpc_filepath.pop(); //build
            rpc_filepath.pop(); //cargo
            rpc_filepath.pop(); //root
            rpc_filepath.push("nano_rpc");
            if let Some(ext) = running_executable_filepath.extension() {
                rpc_filepath.set_extension(ext);
            }
            rpc_filepath
        }
    };

    if !rpc_path.exists() {
        panic!("nano_rpc executable could not be found in {:?}", rpc_path);
    }

    let mut data_paths = Vec::new();
    for i in 0..node_count {
        let data_path = unique_path().ok_or_else(|| anyhow!("no unique path"))?;
        std::fs::create_dir(data_path.as_path())?;
        write_config_files(data_path.as_path(), i)?;
        data_paths.push(data_path);
    }

    let current_network = DEV_NETWORK_PARAMS.network.get_current_network_as_string();
    let mut nodes: Vec<Child> = Vec::new();
    let mut rpc_servers: Vec<Child> = Vec::new();
    for data_path in &data_paths {
        nodes.push(
            Command::new(node_path.as_os_str())
                .arg("--daemon")
                .arg("--data_path")
                .arg(data_path)
                .arg("--network")
                .arg(current_network)
                .spawn()
                .expect("could not spawn node"),
        );
        rpc_servers.push(
            Command::new(rpc_path.as_os_str())
                .arg("--daemon")
                .arg("--data_path")
                .arg(data_path)
                .arg("--network")
                .arg(current_network)
                .spawn()
                .expect("could not spawn rpc server"),
        );
    }

    println!("Waiting for nodes to spin up...");
    std::thread::sleep(Duration::from_secs(7));
    println!("Connecting nodes...");

    // 	boost::asio::io_context ioc;
    // 	debug_assert (!nano::signal_handler_impl);
    // 	nano::signal_handler_impl = [&ioc] () {
    // 		ioc.stop ();
    // 	};

    // 	std::signal (SIGINT, &nano::signal_handler);
    // 	std::signal (SIGTERM, &nano::signal_handler);

    // 	tcp::resolver resolver{ ioc };
    // 	auto const primary_node_results = resolver.resolve ("::1", std::to_string (rpc_port_start));

    // 	std::thread t ([send_count, &ioc, &primary_node_results, &resolver, &node_count, &destination_count] () {
    // 		for (int i = 0; i < node_count; ++i)
    // 		{
    // 			keepalive_rpc (ioc, primary_node_results, peering_port_start + i);
    // 		}

    // 		std::cout << "Beginning tests" << std::endl;

    // 		// Create keys
    // 		std::vector<account> destination_accounts;
    // 		for (int i = 0; i < destination_count; ++i)
    // 		{
    // 			destination_accounts.emplace_back (key_create_rpc (ioc, primary_node_results));
    // 		}

    // 		// Create wallet
    // 		std::string wallet = wallet_create_rpc (ioc, primary_node_results);

    // 		// Add genesis account to it
    // 		wallet_add_rpc (ioc, primary_node_results, wallet, nano::dev::genesis_key.prv.to_string ());

    // 		// Add destination accounts
    // 		for (auto & account : destination_accounts)
    // 		{
    // 			wallet_add_rpc (ioc, primary_node_results, wallet, account.private_key);
    // 		}

    // 		std::cout << "\rPrimary node processing transactions: 00%";

    // 		std::random_device rd;
    // 		std::mt19937 mt (rd ());
    // 		std::uniform_int_distribution<size_t> dist (0, destination_accounts.size () - 1);

    // 		std::atomic<int> send_calls_remaining{ send_count };

    // 		for (auto i = 0; i < send_count; ++i)
    // 		{
    // 			account * destination_account;
    // 			if (i < destination_accounts.size ())
    // 			{
    // 				destination_account = &destination_accounts[i];
    // 			}
    // 			else
    // 			{
    // 				auto random_account_index = dist (mt);
    // 				destination_account = &destination_accounts[random_account_index];
    // 			}

    // 			// Send from genesis account to different accounts and receive the funds
    // 			boost::asio::spawn (ioc, [&ioc, &primary_node_results, &wallet, destination_account, &send_calls_remaining] (boost::asio::yield_context yield) {
    // 				send_receive (ioc, wallet, nano::dev::genesis->account ().to_account (), destination_account->as_string, send_calls_remaining, primary_node_results, yield);
    // 			});
    // 		}

    // 		while (send_calls_remaining != 0)
    // 		{
    // 			static int last_percent = 0;
    // 			auto percent = static_cast<int> (100 * ((send_count - send_calls_remaining) / static_cast<double> (send_count)));

    // 			if (last_percent != percent)
    // 			{
    // 				std::cout << "\rPrimary node processing transactions: " << std::setfill ('0') << std::setw (2) << percent << "%";
    // 				last_percent = percent;
    // 			}
    // 		}

    // 		std::cout << "\rPrimary node processed transactions                " << std::endl;

    // 		std::cout << "Waiting for nodes to catch up..." << std::endl;

    // 		std::map<std::string, account_info> known_account_info;
    // 		for (int i = 0; i < destination_accounts.size (); ++i)
    // 		{
    // 			known_account_info.emplace (destination_accounts[i].as_string, account_info_rpc (ioc, primary_node_results, destination_accounts[i].as_string));
    // 		}

    // 		nano::timer<std::chrono::milliseconds> timer;
    // 		timer.start ();

    // 		for (int i = 1; i < node_count; ++i)
    // 		{
    // 			auto const results = resolver.resolve ("::1", std::to_string (rpc_port_start + i));
    // 			for (auto & account_info : known_account_info)
    // 			{
    // 				while (true)
    // 				{
    // 					auto other_account_info = account_info_rpc (ioc, results, account_info.first);
    // 					if (!other_account_info.error && account_info.second == other_account_info)
    // 					{
    // 						// Found the account in this node
    // 						break;
    // 					}

    // 					if (timer.since_start () > std::chrono::seconds (120))
    // 					{
    // 						throw std::runtime_error ("Timed out");
    // 					}

    // 					std::this_thread::sleep_for (std::chrono::seconds (1));
    // 				}
    // 			}

    // 			stop_rpc (ioc, results);
    // 		}

    // 		// Stop main node
    // 		stop_rpc (ioc, primary_node_results);
    // 	});
    // 	nano::thread_runner runner (ioc, simultaneous_process_calls);
    // 	t.join ();
    // 	runner.join ();

    // 	for (auto & node : nodes)
    // 	{
    // 		node->wait ();
    // 	}
    // 	for (auto & rpc_server : rpc_servers)
    // 	{
    // 		rpc_server->wait ();
    // 	}

    // 	std::cout << "Done!" << std::endl;
    Ok(())
}