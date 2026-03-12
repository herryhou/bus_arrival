## Core Data Flow

```txt
  NMEA Input (1 Hz)                    Route Data (route.json)
       │                                     │
       ▼                                     ▼
  ┌─────────────┐                   ┌──────────────┐
  │ NMEA Parser │                   │ Route Loader │
  └─────────────┘                   └──────────────┘
       │                                     │
       ▼                                     ▼
GPS (lat/lon, heading, speed)      RouteNode[], Stop[]
       │                                     │
       └──────────────┬──────────────────────┘
                      ▼
            ┌─────────────────────┐
            │ Map Matching        │  ← Grid index + heading filter
            │ (best segment)      │
            └─────────────────────┘
                      │
                      ▼
            ┌─────────────────────┐
            │ Segment Projection  │  ← GPS → route progress z (cm)
            └─────────────────────┘
                      │
                      ▼
            ┌─────────────────────┐
            │ Speed Filter        │  ← Reject jumps > 3667 cm
            └─────────────────────┘
                      │
                      ▼
            ┌─────────────────────┐
            │ Kalman Filter (1D)  │  ← Smooth ŝ, v̂
            └─────────────────────┘
                      │
                      ▼
            ┌─────────────────────┐
            │ Stop Corridor       │  ← Find active stop
            └─────────────────────┘
                      │
                      ▼
            ┌─────────────────────┐
            │ Probability Model   │  ← 4-feature fusion
            └─────────────────────┘
                      │
                      ▼
            ┌─────────────────────┐
            │ State Machine       │  ← FSM transition
            └─────────────────────┘
                      │
                      ▼
                Arrival Event
```

## Phase 1: Processing Pipeline:
```txt


  route.json + stops.json
         │
         ▼
  ┌─────────────────────┐
  │ 1. Douglas-Peucker  │  ε = 700cm, curve protection ε=250cm
  └─────────────────────┘
         │
         ▼
  ┌─────────────────────┐
  │ 2. Coordinate Conv  │  lat/lon → x_cm, y_cm (planar approx)
  └─────────────────────┘
         │
         ▼
  ┌─────────────────────┐
  │ 3. Route Linearize  │  cum_dist, dx/dy, len2, line_a/b/c, heading
  └─────────────────────┘
         │
         ▼
  ┌─────────────────────┐
  │ 4. Stop Projection  │  stops → progress_cm, corridor boundaries
  └─────────────────────┘
         │
         ▼
  ┌─────────────────────┐
  │ 5. Grid Index       │  100m cells, segment lists
  └─────────────────────┘
         │
         ▼
     route_data.bin
```

## Phase 2:   Data Flow

```txt
  NMEA Input (1 Hz)
      │
      ▼
  [NMEA Parser] → GPS Point (lat, lon, heading, speed, HDOP)
      │
      ▼
  [route_data.bin Reader] → RouteNode[], Stop[], GridOrigin
      │
      ▼
  [Build Grid Index] → spatial lookup table
      │
      ▼
  [Map Matching] → best segment (heading-constrained)
      │
      ▼
  [Segment Projection] → z_cm (raw route progress)
      │
      ▼
  [Speed Filter] → reject/accept
      │
      ▼
  [Kalman Filter] → ŝ_cm, v̂_cms (smoothed)
      │
      ▼
  [Dead-Reckoning] → (active during GPS outage)
      │
      ▼
  JSON Output per GPS update
```