
## use const as possible
````rust
// better
const FIXED_ORIGIN_LAT_RAD: f64 = FIXED_ORIGIN_LAT_DEG.to_radians();
// worse
let fixed_origin_lat_rad = FIXED_ORIGIN_LAT_DEG.to_radians();
````    