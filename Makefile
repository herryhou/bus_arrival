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
DATA_DIR := test_data
GEN_NMEA := $(TOOLS_DIR)/gen_nmea/gen_nmea.js

# Rust binaries (built with cargo)
PREPROCESSOR := target/release/preprocessor
PIPELINE := target/release/pipeline
TRACE_VALIDATOR := target/release/trace_validator

# Pico 2 W firmware (RP2350, Cortex-M33)
FIRMWARE := target/thumbv8m.main-none-eabi/release/pico2-firmware
FIRMWARE_UF2 := target/pico2-firmware.uf2
FIRMWARE_ELF := target/thumbv8m.main-none-eabi/release/pico2-firmware

# Deprecated: Use unified pipeline instead
# SIMULATOR := target/release/simulator
# ARRIVAL_DETECTOR := target/release/arrival_detector

# Route configuration (can be overridden)
ROUTE_NAME ?= ty225
SCENARIO ?= normal

# Input files
ROUTE_JSON := $(DATA_DIR)/$(ROUTE_NAME)_route.json
STOPS_JSON := $(DATA_DIR)/$(ROUTE_NAME)_stops.json

# Output files (named by route and scenario)
NMEA_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_nmea.txt
ROUTE_DATA_BIN := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO).bin
# SIMULATOR_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_sim.json  # Deprecated
DETECTOR_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_arrivals.json
TRACE_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_trace.jsonl
ANNOUNCE_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_announce.jsonl

# Node.js executable
NODE := node

.PHONY: all run gen_nmea preprocess simulate detect pipeline clean help build validate-trace validate-ty225 validate-all build-firmware firmware-uf2 flash-firmware

# Default target
all: run

# Main pipeline: run unified pipeline (recommended)
run: build gen_nmea preprocess pipeline
	@echo ""
	@echo "=== Pipeline Complete ==="
	@echo "Route: $(ROUTE_NAME)"
	@echo "Scenario: $(SCENARIO)"
	@echo "NMEA output: $(NMEA_OUT)"
	@echo "Route data: $(ROUTE_DATA_BIN)"
	@echo "Output: $(DETECTOR_OUT)"
	@echo "Trace output: $(TRACE_OUT)"
	@echo "Announce output: $(ANNOUNCE_OUT)"

# Legacy two-step workflow (deprecated - use 'make run' instead)
run-legacy: build gen_nmea preprocess simulate detect
	@echo ""
	@echo "=== Legacy Pipeline Complete ==="
	@echo "Route: $(ROUTE_NAME)"
	@echo "Scenario: $(SCENARIO)"
	@echo "NMEA output: $(NMEA_OUT)"
	@echo "Route data: $(ROUTE_DATA_BIN)"
	@echo "Simulator output: $(SIMULATOR_OUT)"
	@echo "Arrival detector output: $(DETECTOR_OUT)"
	@echo "Trace output: $(TRACE_OUT)"
	@echo "Announce output: $(ANNOUNCE_OUT)"

# Build all Rust binaries in release mode
build: build-firmware
	@echo "=== Building Rust host binaries ==="
	cargo build --release --bin pipeline --bin preprocessor --target $(shell rustc -vV | grep "host:" | awk '{print $$2}')

# Build Pico 2 W firmware (true no_std with embassy-rp)
# Target: thumbv8m.main-none-eabi (RP2350, ARM Cortex-M33)
# Uses embassy-rp async framework with defmt logging
build-firmware:
	@echo "=== Building Pico 2 W firmware (no_std, embassy-rp) ==="
	cargo build --release --target thumbv8m.main-none-eabi -p pico2-firmware
	@echo "Firmware binary: $(FIRMWARE)"

# Convert firmware ELF to UF2 format for USB flashing
firmware-uf2: build-firmware
	@echo "=== Converting firmware to UF2 format ==="
	@if command -v elf2uf2-rs >/dev/null 2>&1; then \
		elf2uf2-rs $(FIRMWARE_ELF) $(FIRMWARE_UF2); \
	else \
		echo "Error: elf2uf2-rs not found. Install with: cargo install elf2uf2-rs"; \
		exit 1; \
	fi
	@echo "Firmware UF2: $(FIRMWARE_UF2)"

# Flash firmware to connected Pico 2 W via probe-rs
flash-firmware: build-firmware
	@echo "=== Flashing firmware to Pico 2 W (RP2350) ==="
	@if command -v probe-rs >/dev/null 2>&1; then \
		probe-rs flash $(FIRMWARE_ELF) --chip RP2350; \
	else \
		echo "Error: probe-rs not found. Install with: cargo install probe-rs --features-cli"; \
		exit 1; \
	fi

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

# Run simulator: NMEA + route_data → GPS trace (DEPRECATED)
simulate: gen_nmea preprocess
	@echo "=== Running simulator (DEPRECATED) ==="
	@echo "Note: simulator binary is deprecated. Use the unified pipeline:"
	@echo "  make pipeline ROUTE_NAME=$(ROUTE_NAME) SCENARIO=$(SCENARIO)"
	@false

# Run arrival detector: GPS trace + route_data → arrivals + announce (DEPRECATED)
detect: simulate
	@echo "=== Running arrival detector (DEPRECATED) ==="
	@echo "Note: arrival_detector binary is deprecated. Use the unified pipeline:"
	@echo "  make pipeline ROUTE_NAME=$(ROUTE_NAME) SCENARIO=$(SCENARIO)"
	@false

# Run unified pipeline: NMEA + route_data → arrivals + departures (single binary)
pipeline: gen_nmea preprocess
	@echo "=== Running unified pipeline ==="
	@echo "Binary: $(PIPELINE)"
	@echo "Source: pipeline/"
	$(PIPELINE) $(NMEA_OUT) $(ROUTE_DATA_BIN) $(DETECTOR_OUT) --trace $(TRACE_OUT) --announce $(ANNOUNCE_OUT)
	@echo "Generated: $(DETECTOR_OUT)"
	@echo "Generated: $(TRACE_OUT)"
	@echo "Generated: $(ANNOUNCE_OUT)"

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
	@echo "Pipeline Usage:"
	@echo "  make run ROUTE_NAME=<route> SCENARIO=<name>     Run full unified pipeline"
	@echo "                                                    (default: ROUTE_NAME=ty225 SCENARIO=normal)"
	@echo "  make pipeline ROUTE_NAME=<route> SCENARIO=<name> Run unified pipeline (same as 'run')"
	@echo "  make gen_nmea ROUTE_NAME=<route> SCENARIO=<name> Generate NMEA test data"
	@echo "  make preprocess ROUTE_NAME=<route>               Generate route_data.bin"
	@echo ""
	@echo "Build Targets:"
	@echo "  make build                                       Build all Rust binaries (host + firmware)"
	@echo "  make build-firmware                              Build Pico 2 W firmware (no_std, embassy-rp)"
	@echo "  make firmware-uf2                                 Convert firmware to UF2 for USB flashing"
	@echo "  make flash-firmware                              Flash firmware to connected Pico 2 W (RP2350)"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean                                       Remove generated files"
	@echo "  make help                                        Show this help message"
	@echo ""
	@echo "Parameters:"
	@echo "  ROUTE_NAME    Route identifier (default: ty225)"
	@echo "                Expects files: test_data/<ROUTE_NAME>_route.json"
	@echo "                           and test_data/<ROUTE_NAME>_stops.json"
	@echo "  SCENARIO      Test scenario: normal, drift, jump, outage (default: normal)"
	@echo ""
	@echo "Examples:"
	@echo "  make run ROUTE_NAME=ty225 SCENARIO=normal"
	@echo "  make run ROUTE_NAME=ty225 SCENARIO=drift"
	@echo "  make pipeline ROUTE_NAME=tpF805 SCENARIO=normal"
	@echo ""
	@echo "Firmware:"
	@echo "  make build-firmware        # Build firmware (no_std, embassy-rp, RP2350)"
	@echo "  make firmware-uf2           # Create UF2 for drag-and-drop flashing"
	@echo "  make flash-firmware         # Flash via probe-rs to RP2350"
	@echo ""
	@echo "Note: Firmware uses embassy-rp async framework with true no_std support."

# Trace validation targets
.PHONY: validate-trace validate-ty225 validate-all

validate-trace:
	@if [ -n "$(GROUND_TRUTH)" ]; then \
		cargo run --release --bin trace_validator -- "$(TRACE_FILE)" --ground-truth "$(GROUND_TRUTH)" -o "$(OUTPUT)"; \
	else \
		cargo run --release --bin trace_validator -- "$(TRACE_FILE)" -o "$(OUTPUT)"; \
	fi

validate-ty225:
	@cargo run --release --bin trace_validator -- \
		test_data/tpF805_normal_trace.jsonl \
		--ground-truth ground_truth.json \
		-o validation_report.html \
		--verbose

validate-all:
	@for trace in visualizer/static/*_trace.jsonl; do \
		output=$${trace%_trace.jsonl}_report.html; \
		cargo run --release --bin trace_validator -- "$$trace" -o "$$output"; \
	done
