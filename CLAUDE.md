Always follow the docs/bus_arrival_tech_report_v8.md, which is the source of truth for the arrival detection system design and implementation.
The test plan should be comprehensive, covering all critical scenarios, edge cases, and constraints outlined in the technical report.

## Source of Truth Documents

### Primary
- **bus_arrival_tech_report_v8.md** - Complete system design, algorithms, and constraints

### Binary Format Specifications
- **spatial_grid_binary_format.md** - Grid index v5.1 format (sparse bitmask + u16 offsets, XIP support)
- **crates/shared/src/binfile.rs** - Reference implementation for read/write

### Related Tech Notes
- **dev_guide.md** - Development setup and workflows
- **arrival_detector_test.md** - Test case generation guidelines