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

**See:** `docs/bus_arrival_tech_report_v8.md#appendix-b` for full procedure
