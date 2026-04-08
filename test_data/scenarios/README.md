# Scenario Test Data

This directory contains test data for scenario-based integration tests.

## Organization

Test data is organized by scenario type:
- `normal/` - Normal operation scenarios
- `drift/` - GPS drift scenarios
- `jump/` - GPS jump scenarios
- `outage/` - Signal loss scenarios
- `edge_cases/` - Route geometry edge cases

## Current State

For now, scenario tests use existing test data:
- `../ty225_normal.*` - Normal operation
- `../ty225_drift.*` - GPS drift
- `../ty225_jump.*` - GPS jump
- `../ty225_outage.*` - Signal outage

## Future

Additional scenario-specific test data will be organized here.
