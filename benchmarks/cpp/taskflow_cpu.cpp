#include <algorithm>
#include <bit>
#include <chrono>
#include <cstdint>
#include <functional>
#include <iostream>
#include <limits>
#include <numeric>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

#include <taskflow/taskflow.hpp>
#include <taskflow/algorithm/find.hpp>
#include <taskflow/algorithm/reduce.hpp>
#include <taskflow/algorithm/scan.hpp>
#include <taskflow/algorithm/sort.hpp>
#include <taskflow/algorithm/transform.hpp>

constexpr std::uint64_t FIND_NEEDLE = 0x9e3779b97f4a7c15ULL;
constexpr std::uint64_t CHECKSUM_OFFSET = 1469598103934665603ULL;
constexpr std::uint64_t CHECKSUM_PRIME = 1099511628211ULL;

std::uint64_t transform_value(std::uint64_t value) {
  return std::rotl(value, 7) + 0x9e3779b9ULL;
}

struct TransformOp {
  std::uint64_t operator()(std::uint64_t value) const {
    return transform_value(value);
  }
};

struct FindNeedleOp {
  bool operator()(std::uint64_t value) const {
    return value == FIND_NEEDLE;
  }
};

std::uint64_t checksum_slice(const std::vector<std::uint64_t>& values) {
  std::uint64_t acc = CHECKSUM_OFFSET;
  for(auto value : values) {
    acc = (acc * CHECKSUM_PRIME) ^ value;
  }
  return acc;
}

std::vector<std::uint64_t> make_input(std::size_t len, std::uint64_t seed) {
  std::uint64_t state = seed | 1ULL;
  std::vector<std::uint64_t> values;
  values.reserve(len);

  for(std::size_t i = 0; i < len; ++i) {
    state = state * 6364136223846793005ULL + 1442695040888963407ULL;
    std::uint64_t value = state ^ (state >> 33);
    if(value == FIND_NEEDLE) {
      value ^= 0xa0761d6478bd642fULL;
    }
    values.push_back(value);
  }

  if(!values.empty()) {
    values[(len * 3) / 4] = FIND_NEEDLE;
  }

  return values;
}

std::string next_arg(
  int& index,
  int argc,
  char** argv,
  const std::string& flag
) {
  if(index + 1 >= argc) {
    throw std::runtime_error("missing value for " + flag);
  }
  return argv[++index];
}

int main(int argc, char** argv) {
  std::string scenario = "reduce";
  std::size_t input_len = 65536;
  std::size_t workers = 4;
  std::size_t chunk_size = 1024;
  std::size_t warmups = 1;
  std::size_t iterations = 5;
  std::uint64_t seed = 42;

  try {
    for(int index = 1; index < argc; ++index) {
      std::string flag = argv[index];
      if(flag == "--scenario") {
        scenario = next_arg(index, argc, argv, flag);
      }
      else if(flag == "--input-len") {
        input_len = std::stoull(next_arg(index, argc, argv, flag));
      }
      else if(flag == "--workers") {
        workers = std::stoull(next_arg(index, argc, argv, flag));
      }
      else if(flag == "--chunk-size") {
        chunk_size = std::stoull(next_arg(index, argc, argv, flag));
      }
      else if(flag == "--warmups") {
        warmups = std::stoull(next_arg(index, argc, argv, flag));
      }
      else if(flag == "--iterations") {
        iterations = std::stoull(next_arg(index, argc, argv, flag));
      }
      else if(flag == "--seed") {
        seed = std::stoull(next_arg(index, argc, argv, flag));
      }
      else {
        throw std::runtime_error("unknown argument " + flag);
      }
    }

    std::vector<std::uint64_t> base = make_input(input_len, seed);
    tf::Executor executor(workers);
    std::vector<long long> samples_ns;
    samples_ns.reserve(iterations);
    std::uint64_t checksum = 0;

    auto run_once = [&]() -> std::uint64_t {
      tf::Taskflow flow;

      if(scenario == "transform") {
        std::vector<std::uint64_t> output(base.size());
        flow.transform(
          base.begin(),
          base.end(),
          output.begin(),
          TransformOp{},
          tf::StaticPartitioner(chunk_size)
        );
        executor.run(flow).wait();
        return checksum_slice(output);
      }

      if(scenario == "reduce") {
        std::uint64_t result = 0;
        flow.reduce(
          base.begin(),
          base.end(),
          result,
          std::plus<std::uint64_t>{},
          tf::StaticPartitioner(chunk_size)
        );
        executor.run(flow).wait();
        return result;
      }

      if(scenario == "find") {
        auto found = base.end();
        flow.find_if(
          base.begin(),
          base.end(),
          found,
          FindNeedleOp{},
          tf::StaticPartitioner(chunk_size)
        );
        executor.run(flow).wait();
        if(found == base.end()) {
          return std::numeric_limits<std::uint64_t>::max();
        }
        return static_cast<std::uint64_t>(std::distance(base.begin(), found));
      }

      if(scenario == "inclusive-scan") {
        std::vector<std::uint64_t> output(base.size());
        flow.inclusive_scan(
          base.begin(),
          base.end(),
          output.begin(),
          std::plus<std::uint64_t>{}
        );
        executor.run(flow).wait();
        return checksum_slice(output);
      }

      if(scenario == "sort") {
        std::vector<std::uint64_t> output = base;
        flow.sort(output.begin(), output.end());
        executor.run(flow).wait();
        return checksum_slice(output);
      }

      throw std::runtime_error("unknown scenario " + scenario);
    };

    for(std::size_t i = 0; i < warmups; ++i) {
      checksum = run_once();
    }

    for(std::size_t i = 0; i < iterations; ++i) {
      auto start = std::chrono::steady_clock::now();
      checksum = run_once();
      auto end = std::chrono::steady_clock::now();
      samples_ns.push_back(
        std::chrono::duration_cast<std::chrono::nanoseconds>(end - start).count()
      );
    }

    std::cout << "checksum " << checksum << '\n';
    for(auto sample : samples_ns) {
      std::cout << "sample_ns " << sample << '\n';
    }
  }
  catch(const std::exception& error) {
    std::cerr << error.what() << '\n';
    return 1;
  }

  return 0;
}
