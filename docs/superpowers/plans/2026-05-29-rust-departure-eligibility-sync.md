# Rust Departure Eligibility Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep Rust host and firmware stop FSMs eligible long enough to emit `Departed` after the strict corridor end.

**Architecture:** Leave `pipeline_filter::active_stops()` as a strict corridor-only helper. Add small orchestration-local predicates in the host pipeline and firmware control layer that also include stops whose FSM state is `Arriving` or `AtStop`. Do not add timeout behavior in this change; normal `Departed` transition remains the cleanup path.

**Tech Stack:** Rust, `detection::state_machine::StopState`, `shared::FsmState`, host `Vec`, firmware `heapless::Vec`, existing cargo tests via `rtk`.

---

### Task 1: Host Pipeline Lifecycle Eligibility

**Files:**
- Modify: `crates/pipeline/src/detection_state.rs`

- [ ] **Step 1: Add failing host tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `crates/pipeline/src/detection_state.rs`:

```rust
#[test]
fn process_gps_record_emits_departure_after_corridor_end_for_arriving_stop() {
    let route_data = load_route_data();
    let mut state = DetectionState::new(&route_data);
    let mut result = PipelineResult::new();
    let stop = &route_data.stops()[0];

    state.stop_states[0].fsm_state = shared::FsmState::Arriving;

    let record = gps_record("valid", stop.progress_cm + 4001);
    assert!(record.s_cm > stop.corridor_end_cm);

    state.process_gps_record(&record, &route_data, &mut result);

    assert_eq!(state.active_indices(), &[0]);
    assert_eq!(result.departures.len(), 1);
    assert_eq!(result.departures[0].stop_idx, 0);
    assert_eq!(state.stop_states[0].fsm_state, shared::FsmState::Departed);
}

#[test]
fn process_gps_record_keeps_corridor_end_boundary_inclusive() {
    let route_data = load_route_data();
    let mut state = DetectionState::new(&route_data);
    let mut result = PipelineResult::new();
    let stop = &route_data.stops()[0];

    let record = gps_record("valid", stop.corridor_end_cm);
    state.process_gps_record(&record, &route_data, &mut result);

    assert_eq!(state.active_indices(), &[0]);
}

#[test]
fn process_gps_record_excludes_post_corridor_idle_and_departed_stops() {
    let route_data = load_route_data();
    let mut state = DetectionState::new(&route_data);
    let mut result = PipelineResult::new();
    let stop = &route_data.stops()[0];
    let record = gps_record("valid", stop.corridor_end_cm + 1);

    state.stop_states[0].fsm_state = shared::FsmState::Idle;
    state.process_gps_record(&record, &route_data, &mut result);
    assert!(state.active_indices().is_empty());

    state.stop_states[0].fsm_state = shared::FsmState::Departed;
    state.process_gps_record(&record, &route_data, &mut result);
    assert!(state.active_indices().is_empty());
}
```

- [ ] **Step 2: Run host test to verify the departure case fails**

Run:

```bash
rtk cargo test -p pipeline process_gps_record_emits_departure_after_corridor_end_for_arriving_stop
```

Expected before implementation: FAIL because `active_indices` is empty and no departure is emitted.

- [ ] **Step 3: Add the host predicate and use it for active selection**

In `crates/pipeline/src/detection_state.rs`, import `Stop` and `FsmState`:

```rust
use shared::{DistCm, FsmState, PositionSignals, Prob8, Stop, TimestampMs};
```

Add this helper near the top-level impl area:

```rust
fn should_update_stop_for_detection(s_cm: DistCm, stop: &Stop, stop_state: &StopState) -> bool {
    let in_corridor = s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm;
    let awaiting_departure = matches!(stop_state.fsm_state, FsmState::Arriving | FsmState::AtStop);
    in_corridor || awaiting_departure
}
```

Replace the active-stop selection condition with:

```rust
if should_update_stop_for_detection(s_cm, stop, &self.stop_states[idx])
    && !self.stop_states[idx].skip_on_reentry
{
    self.active_indices.push(idx);
}
```

- [ ] **Step 4: Run the host tests**

Run:

```bash
rtk cargo test -p pipeline process_gps_record_emits_departure_after_corridor_end_for_arriving_stop
rtk cargo test -p pipeline process_gps_record_keeps_corridor_end_boundary_inclusive
rtk cargo test -p pipeline process_gps_record_excludes_post_corridor_idle_and_departed_stops
```

Expected: all pass.

- [ ] **Step 5: Commit host change**

Run:

```bash
rtk git add crates/pipeline/src/detection_state.rs
rtk git commit -m "Sync host departure eligibility"
```

### Task 2: Firmware Lifecycle Eligibility

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

- [ ] **Step 1: Add failing firmware predicate tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `crates/pico2-firmware/src/control/mod.rs`:

```rust
#[test]
fn should_update_stop_for_detection_keeps_departure_states_after_corridor_end() {
    use shared::{FsmState, Stop};

    let stop = Stop {
        progress_cm: 10_000,
        corridor_start_cm: 5_000,
        corridor_end_cm: 12_000,
    };

    assert!(should_update_stop_for_detection(12_001, &stop, FsmState::Arriving));
    assert!(should_update_stop_for_detection(12_001, &stop, FsmState::AtStop));
    assert!(!should_update_stop_for_detection(12_001, &stop, FsmState::Idle));
    assert!(!should_update_stop_for_detection(12_001, &stop, FsmState::Departed));
}

#[test]
fn should_update_stop_for_detection_keeps_corridor_end_boundary_inclusive() {
    use shared::{FsmState, Stop};

    let stop = Stop {
        progress_cm: 10_000,
        corridor_start_cm: 5_000,
        corridor_end_cm: 12_000,
    };

    assert!(should_update_stop_for_detection(12_000, &stop, FsmState::Idle));
}
```

- [ ] **Step 2: Run firmware test to verify helper is missing**

Run:

```bash
rtk cargo test -p pico2-firmware should_update_stop_for_detection_keeps_departure_states_after_corridor_end
```

Expected before implementation: FAIL because `should_update_stop_for_detection` does not exist.

- [ ] **Step 3: Add the firmware predicate and use a direct bounded scan**

In `crates/pico2-firmware/src/control/mod.rs`, import `FsmState` and `Stop`:

```rust
use shared::{binfile::RouteData, ArrivalEvent, DistCm, FsmState, GpsPoint, Stop};
```

Add this helper near `run_detection` or before `impl SystemState`:

```rust
fn should_update_stop_for_detection(s_cm: DistCm, stop: &Stop, fsm_state: FsmState) -> bool {
    let in_corridor = s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm;
    let awaiting_departure = matches!(fsm_state, FsmState::Arriving | FsmState::AtStop);
    in_corridor || awaiting_departure
}
```

Replace the `find_active_stops` call in `run_detection` with a direct bounded scan:

```rust
let mut active_indices = heapless::Vec::<usize, 16>::new();
for stop_idx in 0..self.route_data.stop_count as usize {
    if stop_idx >= self.stop_states.len() {
        continue;
    }

    let stop = match self.route_data.get_stop(stop_idx) {
        Some(stop) => stop,
        None => continue,
    };

    if should_update_stop_for_detection(est.s_cm, &stop, self.stop_states[stop_idx].fsm_state)
        && active_indices.push(stop_idx).is_err()
    {
        #[cfg(feature = "firmware")]
        defmt::warn!("Active stops overflow (>16), truncating");
        break;
    }
}
```

Remove the local `skip_flags` variable. Do not change skip-on-reentry behavior beyond the current firmware behavior, which does not yet track skip flags.

- [ ] **Step 4: Run the firmware tests**

Run:

```bash
rtk cargo test -p pico2-firmware should_update_stop_for_detection_keeps_departure_states_after_corridor_end
rtk cargo test -p pico2-firmware should_update_stop_for_detection_keeps_corridor_end_boundary_inclusive
```

Expected: both pass.

- [ ] **Step 5: Commit firmware change**

Run:

```bash
rtk git add crates/pico2-firmware/src/control/mod.rs
rtk git commit -m "Sync firmware departure eligibility"
```

### Task 3: Documentation and Regression Verification

**Files:**
- Modify: `docs/specs/06-state_machine.md`
- Modify: `docs/superpowers/specs/2026-05-29-rust-departure-eligibility-sync-design.md`
- Create: `docs/superpowers/plans/2026-05-29-rust-departure-eligibility-sync.md`

- [ ] **Step 1: Update state-machine docs**

In `docs/specs/06-state_machine.md`, add a concise note near active stop selection or lifecycle events:

```markdown
Host and firmware orchestration keep stops in `Arriving` or `AtStop` eligible
for FSM updates after `corridor_end_cm` so the FSM can observe and emit
`Departed`. The shared corridor filter remains strict and inclusive; idle or
already departed stops outside their corridors are not selected by this lifecycle
rule. No stale-state timeout is added by this behavior.
```

- [ ] **Step 2: Run regression suites**

Run:

```bash
rtk cargo test -p pipeline detection_state
rtk cargo test -p pico2-firmware
rtk cargo test -p detection
rtk cargo test -p pipeline --lib
```

Expected: all pass.

- [ ] **Step 3: Commit docs and plan**

Run:

```bash
rtk git add docs/specs/06-state_machine.md docs/superpowers/specs/2026-05-29-rust-departure-eligibility-sync-design.md docs/superpowers/plans/2026-05-29-rust-departure-eligibility-sync.md
rtk git commit -m "Document Rust departure eligibility sync"
```
