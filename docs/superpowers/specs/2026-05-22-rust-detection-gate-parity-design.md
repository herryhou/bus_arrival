# Rust Detection Gate Parity Design

## Context

Android now gates arrival detection so stop FSM state advances only on trusted
steady-state Normal ticks. The same policy must apply to Rust firmware and the
host Rust pipeline.

The Android rule is:

- Detection is disabled before the first usable fix.
- Detection is disabled in OffRoute and Recovering.
- Detection is disabled during Normal -> OffRoute suspect hysteresis ticks.
- Detection is disabled on the direct OffRoute -> Normal resume tick.
- Detection resumes on the next clean Normal tick.

Rust currently has related state but does not fully enforce the same rule:

- `crates/pico2-firmware/src/control/machine.rs` exposes
  `ModeOutput.detection_enabled`, but suspect Normal ticks and direct
  OffRoute -> Normal resume ticks can still report detection enabled.
- `crates/pico2-firmware/src/control/mod.rs` has duplicated transition logic and
  can continue to call `run_detection()` during suspect Normal ticks after
  warmup.
- `crates/pipeline` also has off-route hysteresis state and must be checked for
  the same stop-FSM advancement behavior.

## Goal

Make Rust behavior match Android: a closed detection gate must prevent arrival
detection from advancing internal stop FSM state, not merely suppress emitted
events.

## Non-Goals

- Do not redesign off-route detection thresholds or hysteresis counts.
- Do not refactor firmware control architecture beyond the gate needed for
  parity.
- Do not introduce a cross-crate abstraction unless the existing crate structure
  makes it clearly smaller than local gates.
- Do not change trace schemas as part of this Rust parity work.

## Detection Gate Rule

Rust should treat detection as allowed only when all of these are true:

1. The current mode is `Normal`.
2. There are no active Normal -> OffRoute suspect ticks.
3. This tick did not directly transition from `OffRoute` or `Recovering` into
   `Normal`.
4. Existing readiness gates, such as warmup and first-fix handling, have passed.

Any tick that fails this rule must skip stop FSM updates. Position estimation,
mode transition counters, recovery checks, and other non-detection bookkeeping
may still run as they do today.

## Firmware Design

### `ModeMachine`

Update `crates/pico2-firmware/src/control/machine.rs` so
`ModeOutput.detection_enabled` follows the same policy:

- In `handle_normal()`, after checking `check_normal_to_offroute(...)`, return
  `detection_enabled = self.off_route_suspect_ticks == 0`.
- If the fifth suspect tick transitions to `OffRoute`, continue returning
  `detection_enabled = false`.
- In `handle_offroute()`, `TransitionAction::ToNormal` should return
  `detection_enabled = false`. Detection resumes on the next clean Normal tick.
- `OffRoute` stay, `ToRecovering`, and `Recovering` remain disabled.

This keeps the pure mode machine honest for any current or future caller.

### `SystemState::tick()`

Update `crates/pico2-firmware/src/control/mod.rs` because it currently has its
own transition logic instead of driving all behavior through `ModeMachine`.

The tick path should compute or preserve a local detection gate that starts
closed and opens only for trusted steady-state Normal ticks. The gate must stay
closed when:

- warmup/readiness returns early,
- mode is `OffRoute` or `Recovering`,
- a Normal tick increments `off_route_suspect_ticks`,
- the tick transitions into `OffRoute`,
- the tick transitions directly from `OffRoute` to `Normal`,
- recovery succeeds and changes mode back to `Normal` on the same tick.

`run_detection(...)` should be called only when this gate is open. Existing
position tracking, monotonic enforcement, recovery, snap handling, and cooldown
logic should remain scoped to their current responsibilities.

Persistence should remain aligned with trusted Normal behavior. Existing
`should_persist()` already rejects OffRoute, Recovering, and suspect ticks; the
implementation should verify that direct resume ticks do not introduce a
persistence leak.

## Host Pipeline Design

Inspect the `crates/pipeline` detection invocation path, especially the modules
around off-route hysteresis and arrival detection. Apply the same gate wherever
the host pipeline can advance stop FSM state.

The implementation should distinguish two cases:

- If host pipeline arrival detection runs while off-route hysteresis is suspect
  or on direct resume ticks, add the same trusted-Normal gate before FSM update.
- If host pipeline only computes localization/off-route state in that path and
  does not advance arrival detection, leave code unchanged and document the
  reason in the implementation summary.

The design requires checking both firmware and host pipeline. It does not
require forcing identical internal structure if one side already has equivalent
behavior.

## Testing

Add or update focused Rust tests before implementation changes.

Firmware `ModeMachine` tests:

- Suspect Normal ticks 1-4 return `mode = Normal` and
  `detection_enabled = false`.
- A clean Normal tick with no suspect ticks returns `detection_enabled = true`.
- The fifth suspect tick transitions to `OffRoute` with
  `detection_enabled = false`.
- Direct `OffRoute -> Normal` returns `detection_enabled = false`.
- The next clean Normal tick returns `detection_enabled = true`.

Firmware orchestrator test:

- Use a small synthetic route/tick sequence that enters suspect Normal while a
  stop would otherwise be active.
- Assert stop FSM state does not advance while the gate is closed.
- Assert the first trusted Normal detection tick starts from the initial stop FSM
  state, matching the Android synthetic pipeline test.

Host pipeline test:

- Add the equivalent synthetic or integration test if the host pipeline has a
  detection path affected by suspect/resume ticks.
- If no affected detection path exists, add no speculative test; document the
  inspected path and why no pipeline change was needed.

Run targeted tests first:

```bash
rtk cargo test -p pico2-firmware
```

If `crates/pipeline` changes:

```bash
rtk cargo test -p pipeline
```

## Risks

- Firmware has duplicated mode logic, so fixing only `ModeMachine` is
  insufficient.
- Host pipeline may encode the gate differently from firmware, so the change
  should be based on the actual detection invocation path rather than only on
  hysteresis variable names.
- Suppressing detection must not suppress position tracking or recovery
  bookkeeping; only stop FSM advancement is gated.
