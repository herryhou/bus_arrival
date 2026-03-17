# Bus Arrival Detection Pipeline Makefile
#
# Usage:
#   make run ROUTE_NAME=ty225 SCENARIO=normal          # Run full pipeline
#   make gen_nmea ROUTE_NAME=ty225 SCENARIO=normal     # Generate NMEA test data only
#   make preprocess ROUTE_NAME=ty225                   # Generate route_data.bin only
#   make simulate ROUTE_NAME=ty225 SCENARIO=normal     # Run simulator only
#   make detect ROUTE_NAME=ty225 SCENARIO=normal       # Run arrival detector only
#   make clean                                         # Clean all generated files

# Configuration
TOOLS_DIR := tools
# DATA_DIR := $(TOOLS_DIR)/data
DATA_DIR := /test_data
GEN_NMEA := $(TOOLS_DIR)/gen_nmea/gen_nmea.js

# Rust binaries (built with cargo)
PREPROCESSOR := target/release/preprocessor
SIMULATOR := target/release/simulator
ARRIVAL_DETECTOR := target/release/arrival_detector

# Route configuration (can be overridden)
ROUTE_NAME ?= ty225
SCENARIO ?= normal

# Input files
ROUTE_JSON := $(DATA_DIR)/$(ROUTE_NAME)_route.json
STOPS_JSON := $(DATA_DIR)/$(ROUTE_NAME)_stops.json

# Output files (named by route and scenario)
NMEA_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_nmea.txt
ROUTE_DATA_BIN := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO).bin
SIMULATOR_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_sim.json
DETECTOR_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_arrivals.json
TRACE_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_trace.json

# Node.js executable
NODE := node

.PHONY: all run gen_nmea preprocess simulate detect clean help build

# Default target
all: run

# Main pipeline: run everything for a named scenario
run: build gen_nmea preprocess simulate detect
	@echo ""
	@echo "=== Pipeline Complete ==="
	@echo "Route: $(ROUTE_NAME)"
	@echo "Scenario: $(SCENARIO)"
	@echo "NMEA output: $(NMEA_OUT)"
	@echo "Route data: $(ROUTE_DATA_BIN)"
	@echo "Simulator output: $(SIMULATOR_OUT)"
	@echo "Arrival detector output: $(DETECTOR_OUT)"
	@echo "Trace output: $(TRACE_OUT)"

# Build all Rust binaries in release mode
build:
	@echo "=== Building Rust binaries ==="
	cargo build --release

# Generate NMEA test data from route and scenario
gen_nmea:
	@echo "=== Generating NMEA test data ==="
	@echo "Route: $(ROUTE_NAME)"
	@echo "Route JSON: $(ROUTE_JSON)"
	@echo "Stops JSON: $(STOPS_JSON)"
	@echo "Scenario: $(SCENARIO)"
	$(NODE) $(GEN_NMEA) generate \
		--route $(ROUTE_JSON) \
		--stops $(STOPS_JSON) \
		--scenario $(SCENARIO) \
		--out_nmea $(NMEA_OUT)
	@echo "Generated: $(NMEA_OUT)"

# Preprocess route and stops into binary route data
preprocess:
	@echo "=== Preprocessing route data ==="
	$(PREPROCESSOR) $(ROUTE_JSON) $(STOPS_JSON) $(ROUTE_DATA_BIN)
	@echo "Generated: $(ROUTE_DATA_BIN)"

# Run simulator: NMEA + route_data → GPS trace
simulate: gen_nmea preprocess
	@echo "=== Running simulator ==="
	$(SIMULATOR) $(NMEA_OUT) $(ROUTE_DATA_BIN) $(SIMULATOR_OUT)
	@echo "Generated: $(SIMULATOR_OUT)"

# Run arrival detector: GPS trace + route_data → arrivals
detect: simulate
	@echo "=== Running arrival detector ==="
	$(ARRIVAL_DETECTOR) $(SIMULATOR_OUT) $(ROUTE_DATA_BIN) $(DETECTOR_OUT) --trace $(TRACE_OUT)
	@echo "Generated: $(DETECTOR_OUT)"
	@echo "Generated: $(TRACE_OUT)"

# Clean all generated files
clean:
	@echo "=== Cleaning generated files ==="
	rm -f $(DATA_DIR)/nmea_*.txt
	rm -f $(DATA_DIR)/sim_*.jsonl
	rm -f $(DATA_DIR)/arrivals_*.jsonl
	rm -f $(DATA_DIR)/trace_*.jsonl
	rm -f $(ROUTE_DATA_BIN)
	@echo "Clean complete"

# Help target
help:
	@echo "Bus Arrival Detection Pipeline"
	@echo ""
	@echo "Usage:"
	@echo "  make run ROUTE_NAME=<route> SCENARIO=<name>     Run full pipeline"
	@echo "                                                    (default: ROUTE_NAME=ty225 SCENARIO=normal)"
	@echo "  make gen_nmea ROUTE_NAME=<route> SCENARIO=<name> Generate NMEA test data"
	@echo "  make preprocess ROUTE_NAME=<route>               Generate route_data.bin"
	@echo "  make simulate ROUTE_NAME=<route> SCENARIO=<name> Run simulator"
	@echo "  make detect ROUTE_NAME=<route> SCENARIO=<name>   Run arrival detector"
	@echo "  make build                                       Build Rust binaries"
	@echo "  make clean                                       Remove generated files"
	@echo "  make help                                        Show this help message"
	@echo ""
	@echo "Parameters:"
	@echo "  ROUTE_NAME    Route identifier (default: ty225)"
	@echo "                Expects files: tools/data/<ROUTE_NAME>_route.json"
	@echo "                           and tools/data/<ROUTE_NAME>_stops.json"
	@echo "  SCENARIO      Test scenario: normal, drift, jump, outage (default: normal)"
	@echo ""
	@echo "Examples:"
	@echo "  make run ROUTE_NAME=ty225 SCENARIO=normal"
	@echo "  make run ROUTE_NAME=ty225 SCENARIO=drift"
	@echo "  make simulate ROUTE_NAME=another_route SCENARIO=jump"
