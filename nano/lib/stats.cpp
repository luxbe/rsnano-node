#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/locks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/tomlconfig.hpp>

#include <boost/format.hpp>
#include <boost/property_tree/json_parser.hpp>

#include <ctime>
#include <fstream>
#include <sstream>

std::string type_to_string (uint32_t key)
{
	uint8_t const * ptr;
	auto len = rsnano::rsn_stat_type_to_string (key, &ptr);
	return std::string (reinterpret_cast<const char *> (ptr), len);
}

std::string detail_key_to_string (uint32_t key)
{
	uint8_t const * ptr;
	auto len = rsnano::rsn_stat_detail_to_string (key, &ptr);
	return std::string (reinterpret_cast<const char *> (ptr), len);
}

std::string dir_to_string (uint32_t key)
{
	uint8_t const * ptr;
	auto len = rsnano::rsn_stat_dir_to_string (key, &ptr);
	return std::string (reinterpret_cast<const char *> (ptr), len);
}

void nano::stat_config::load_dto (rsnano::StatConfigDto & dto)
{
	sampling_enabled = dto.sampling_enabled;
	capacity = dto.capacity;
	interval = dto.interval;
	log_interval_samples = dto.log_interval_samples;
	log_interval_counters = dto.log_interval_counters;
	log_rotation_count = dto.log_rotation_count;
	log_headers = dto.log_headers;
	log_counters_filename = std::string (reinterpret_cast<const char *> (dto.log_counters_filename), dto.log_counters_filename_len);
	log_samples_filename = std::string (reinterpret_cast<const char *> (dto.log_samples_filename), dto.log_samples_filename_len);
}

rsnano::StatConfigDto nano::stat_config::to_dto () const
{
	rsnano::StatConfigDto dto;
	dto.sampling_enabled = sampling_enabled;
	dto.capacity = capacity;
	dto.interval = interval;
	dto.log_interval_samples = log_interval_samples;
	dto.log_interval_counters = log_interval_counters;
	dto.log_rotation_count = log_rotation_count;
	dto.log_headers = log_headers;
	std::copy (log_counters_filename.begin (), log_counters_filename.end (), std::begin (dto.log_counters_filename));
	dto.log_counters_filename_len = log_counters_filename.size ();

	std::copy (log_samples_filename.begin (), log_samples_filename.end (), std::begin (dto.log_samples_filename));
	dto.log_samples_filename_len = log_samples_filename.size ();
	return dto;
}

nano::error nano::stat_config::deserialize_toml (nano::tomlconfig & toml)
{
	auto sampling_l (toml.get_optional_child ("sampling"));
	if (sampling_l)
	{
		sampling_l->get<bool> ("enable", sampling_enabled);
		sampling_l->get<size_t> ("capacity", capacity);
		sampling_l->get<size_t> ("interval", interval);
	}

	auto log_l (toml.get_optional_child ("log"));
	if (log_l)
	{
		log_l->get<bool> ("headers", log_headers);
		log_l->get<size_t> ("interval_counters", log_interval_counters);
		log_l->get<size_t> ("interval_samples", log_interval_samples);
		log_l->get<size_t> ("rotation_count", log_rotation_count);
		log_l->get<std::string> ("filename_counters", log_counters_filename);
		log_l->get<std::string> ("filename_samples", log_samples_filename);

		// Don't allow specifying the same file name for counter and samples logs
		if (log_counters_filename == log_samples_filename)
		{
			toml.get_error ().set ("The statistics counter and samples config values must be different");
		}
	}

	return toml.get_error ();
}

nano::stat_log_sink::stat_log_sink (rsnano::StatLogSinkHandle * handle_a) :
	handle (handle_a)
{
}

nano::stat_log_sink::~stat_log_sink ()
{
	rsnano::rsn_stat_log_sink_destroy (handle);
}

void nano::stat_log_sink::begin ()
{
	rsnano::rsn_stat_log_sink_begin (handle);
}

void nano::stat_log_sink::finalize ()
{
	rsnano::rsn_stat_log_sink_finalize (handle);
}

void nano::stat_log_sink::write_header (std::string const & header, std::chrono::system_clock::time_point & walltime)
{
	rsnano::rsn_stat_log_sink_write_header (handle, header.c_str (), std::chrono::duration_cast<std::chrono::milliseconds> (walltime.time_since_epoch ()).count ());
}

void nano::stat_log_sink::rotate ()
{
	rsnano::rsn_stat_log_sink_rotate (handle);
}

size_t nano::stat_log_sink::entries ()
{
	return rsnano::rsn_stat_log_sink_entries (handle);
}

void nano::stat_log_sink::inc_entries ()
{
	rsnano::rsn_stat_log_sink_inc_entries (handle);
}

std::string nano::stat_log_sink::to_string ()
{
	rsnano::StringDto dto;
	rsnano::rsn_stat_log_sink_to_string (handle, &dto);
	std::string result (dto.value);
	rsnano::rsn_string_destroy (dto.handle);
	return result;
}

void * nano::stat_log_sink::to_object ()
{
	return rsnano::rsn_stat_log_sink_to_object (handle);
}

/** JSON sink. The resulting JSON object is provided as both a property_tree::ptree (to_object) and a string (to_string) */
class json_writer : public nano::stat_log_sink
{
public:
	json_writer () :
		stat_log_sink (rsnano::rsn_json_writer_create ())
	{
	}
};

/** File sink with rotation support. This writes one counter per line and does not include histogram values. */
class file_writer : public nano::stat_log_sink
{
public:
	explicit file_writer (std::string const & filename_a) :
		stat_log_sink{ rsnano::rsn_file_writer_create (reinterpret_cast<const int8_t *> (filename_a.c_str ())) }
	{
	}
};

nano::stat::stat () :
	stat (nano::stat_config ())
{
}

nano::stat::stat (nano::stat_config config)
{
	auto config_dto{ config.to_dto () };
	handle = rsnano::rsn_stat_create (&config_dto);
}

nano::stat::~stat ()
{
	rsnano::rsn_stat_destroy (handle);
}

std::unique_ptr<nano::stat_log_sink> nano::stat::log_sink_json () const
{
	return std::make_unique<json_writer> ();
}

void nano::stat::log_counters (stat_log_sink & sink)
{
	rsnano::rsn_stat_log_counters (handle, sink.handle);
}

void nano::stat::log_samples (stat_log_sink & sink)
{
	rsnano::rsn_stat_log_samples (handle, sink.handle);
}

void nano::stat::define_histogram (stat::type type, stat::detail detail, stat::dir dir, std::initializer_list<uint64_t> intervals_a, size_t bin_count_a /*=0*/)
{
	std::vector<uint64_t> intervals_l{ intervals_a };
	rsnano::rsn_stat_define_histogram (
	handle,
	static_cast<uint8_t> (type),
	static_cast<uint8_t> (detail),
	static_cast<uint8_t> (dir),
	intervals_l.data (),
	intervals_l.size (),
	bin_count_a);
}

void nano::stat::update_histogram (stat::type type, stat::detail detail, stat::dir dir, uint64_t index_a, uint64_t addend_a)
{
	rsnano::rsn_stat_update_histogram (
	handle,
	static_cast<uint8_t> (type),
	static_cast<uint8_t> (detail),
	static_cast<uint8_t> (dir),
	index_a,
	addend_a);
}

std::chrono::seconds nano::stat::last_reset ()
{
	return std::chrono::seconds{ rsnano::rsn_stat_last_reset_s (handle) };
}

void nano::stat::stop ()
{
	rsnano::rsn_stat_stop (handle);
}

void nano::stat::clear ()
{
	rsnano::rsn_stat_clear (handle);
}

std::string nano::stat::detail_to_string (stat::detail detail)
{
	uint8_t const * ptr;
	auto len = rsnano::rsn_stat_type_to_string (static_cast<uint8_t> (detail), &ptr);
	return std::string (reinterpret_cast<const char *> (ptr), len);
}

void nano::stat::configure (stat::type type, stat::detail detail, stat::dir dir, size_t interval, size_t capacity)
{
	rsnano::rsn_stat_configure (
	handle,
	static_cast<uint8_t> (type),
	static_cast<uint8_t> (detail),
	static_cast<uint8_t> (dir),
	interval,
	capacity);
}

void nano::stat::disable_sampling (stat::type type, stat::detail detail, stat::dir dir)
{
	rsnano::rsn_stat_disable_sampling (
	handle,
	static_cast<uint8_t> (type),
	static_cast<uint8_t> (detail),
	static_cast<uint8_t> (dir));
}

void nano::stat::inc (stat::type type, stat::dir dir)
{
	add (type, dir, 1);
}

void nano::stat::inc_detail_only (stat::type type, stat::detail detail, stat::dir dir)
{
	add (type, detail, dir, 1, true);
}

void nano::stat::inc (stat::type type, stat::detail detail, stat::dir dir)
{
	add (type, detail, dir, 1);
}

void nano::stat::add (stat::type type, stat::dir dir, uint64_t value)
{
	add (type, detail::all, dir, value);
}

void nano::stat::add (stat::type type, stat::detail detail, stat::dir dir, uint64_t value, bool detail_only)
{
	rsnano::rsn_stat_add (handle,
	static_cast<uint8_t> (type),
	static_cast<uint8_t> (detail),
	static_cast<uint8_t> (dir),
	value,
	detail_only);
}

uint64_t nano::stat::count (stat::type type, stat::dir dir)
{
	return count (type, stat::detail::all, dir);
}

uint64_t nano::stat::count (stat::type type, stat::detail detail, stat::dir dir)
{
	return rsnano::rsn_stat_count (handle,
	static_cast<uint8_t> (type),
	static_cast<uint8_t> (detail),
	static_cast<uint8_t> (dir));
}

nano::stat_datapoint::stat_datapoint () :
	handle (rsnano::rsn_stat_datapoint_create ())
{
}

nano::stat_datapoint::~stat_datapoint ()
{
	rsnano::rsn_stat_datapoint_destroy (handle);
}

nano::stat_datapoint::stat_datapoint (stat_datapoint const & other_a)
{
	handle = rsnano::rsn_stat_datapoint_clone (other_a.handle);
}

nano::stat_datapoint::stat_datapoint (rsnano::StatDatapointHandle * handle) :
	handle{ handle }
{
}

nano::stat_datapoint & nano::stat_datapoint::operator= (stat_datapoint const & other_a)
{
	handle = rsnano::rsn_stat_datapoint_clone (other_a.handle);
	return *this;
}

uint64_t nano::stat_datapoint::get_value () const
{
	return rsnano::rsn_stat_datapoint_get_value (handle);
}

void nano::stat_datapoint::set_value (uint64_t value_a)
{
	rsnano::rsn_stat_datapoint_set_value (handle, value_a);
}

std::chrono::system_clock::time_point nano::stat_datapoint::get_timestamp () const
{
	auto timestamp_ms = rsnano::rsn_stat_datapoint_get_timestamp_ms (handle);
	return std::chrono::system_clock::time_point{ std::chrono::milliseconds (timestamp_ms) };
}

void nano::stat_datapoint::set_timestamp (std::chrono::system_clock::time_point timestamp_a)
{
	auto timestamp_ms (std::chrono::duration_cast<std::chrono::milliseconds> (timestamp_a.time_since_epoch ()));
	rsnano::rsn_stat_datapoint_set_timestamp_ms (handle, timestamp_ms.count ());
}

/** Add \addend to the current value and optionally update the timestamp */
void nano::stat_datapoint::add (uint64_t addend, bool update_timestamp)
{
	rsnano::rsn_stat_datapoint_add (handle, addend, update_timestamp);
}
