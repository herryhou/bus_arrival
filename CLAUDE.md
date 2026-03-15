Always follow the docs/bus_arrival_tech_report_v8.md, which is the source of truth for the arrival detection system design and implementation. The test plan should be comprehensive, covering all critical scenarios, edge cases, and constraints outlined in the technical report.

## use const as possible
````rust
// better
const FIXED_ORIGIN_LAT_RAD: f64 = FIXED_ORIGIN_LAT_DEG.to_radians();
// worse
let fixed_origin_lat_rad = FIXED_ORIGIN_LAT_DEG.to_radians();
````    