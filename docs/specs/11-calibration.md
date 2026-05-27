# Calibration Procedures

## Overview

This document is for HUMAN developers. LLMs should NOT read this spec — it contains tuning procedures that change with deployment.

## When to Calibrate

- New route deployment
- Algorithm behavior changes
- Accuracy drops below 97% target

## Parameters

| Parameter | Default | Tuning Range |
|-----------|---------|--------------|
| THETA_ARRIVAL | 191 | 180..220 |
| SIGMA_D_CM | 2750 | 2500..3000 |
| SIGMA_P_CM | 2000 | 1500..2500 |
| Probability weights | (13,6,10,3) | Sum = 32 |

## Procedure

1. Collect representative GPS traces for normal runs, close stops, GPS drift,
   GPS outages, and off-route detours.
2. Run the pipeline with trace output enabled.
3. Compare `Announce`, `Arrival`, and `Departure` events against ground truth.
4. Inspect trace fields for false positives and misses: active corridor,
   `z_gps_cm`, `s_cm`, probability features, FSM state, GPS status, and
   divergence.
5. Tune one parameter group at a time and rerun the same traces.
6. Keep changes only when normal-route accuracy remains at or above target and
   edge scenarios do not regress.
