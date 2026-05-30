# Android Detect Tab Redesign

Date: 2026-05-30

## Goal

Redesign the Android Detect tab so it reads as a clean dashboard instead of a dense debug panel. The Detect tab should make the current route, detection state, GPS readiness, current stop, speed, and primary action easy to scan and operate on a tablet.

## Scope

In scope:

- Redesign the Detect tab Compose UI outside `MapView.kt`.
- Update `DetectionScreen.kt` layout only as needed to support the dashboard structure.
- Update `StatusPanel.kt`, `GpsStatusRow.kt`, and `TimelineScrubber.kt` styling/layout as needed.
- Improve permission, no-route, and error states only where they share the Detect tab visual treatment.
- Preserve all existing callbacks, state inputs, and behavior.

Out of scope:

- Any edits to `MapView.kt`.
- Map rendering, map gestures, tile loading, camera follow logic, or route drawing behavior.
- New detection features or replay capabilities.
- Navigation changes outside the Detect tab.

## Design Direction

Use a clean dashboard layout:

- Keep the map as the top region.
- Turn the lower area into a visually quieter dashboard panel.
- Give the current stop and stop state a larger, more prominent treatment.
- Convert GPS, position, and speed into readable metric tiles.
- Make Start/Stop Detection the clear primary action.
- Present GPS logging and camera follow as secondary controls.
- Show recent events as a calmer list with small colored markers and concise labels.
- Restyle the replay scrubber to match the dashboard visual language while preserving behavior.

## Behavior Requirements

- The same `DetectionUiState`, `ReplayState`, `GpsFixState`, and event data must drive the UI.
- Existing start/stop, camera follow, GPS logging, play/pause, seek, speed, and replay camera follow callbacks must remain wired.
- Replay seek must still be disabled for GPS log simulation traces.
- Error dismissal behavior must remain unchanged.
- Permission request behavior must remain unchanged.
- Empty route behavior must remain unchanged.

## Visual Requirements

- Use Material 3 colors and typography.
- Use concrete Material 3 primitives rather than custom drawing where possible:
  `Card`/`ElevatedCard` for dashboard groups, `Button` for the primary Start/Stop action,
  `OutlinedButton` or `TextButton` for secondary actions, `Switch` for binary settings,
  `Slider` for replay position, and `FilterChip` or small `Surface` chips for status labels.
- Keep dashboard card elevation low: use 0-1 dp for regular dashboard surfaces and reserve
  stronger elevation only for overlays such as snackbars or transient hints.
- Avoid nested cards.
- Use stable spacing and dimensions so controls do not shift as values change:
  metric tiles should be at least 56 dp tall, use 8 dp internal spacing, and sit in rows or
  grids with 8-12 dp gaps depending on available width.
- Keep primary action controls at least 48 dp tall.
- Use larger, clearer controls for primary actions.
- Keep text concise and scannable.
- Keep the palette balanced and avoid a single dominant hue.
- Ensure tablet readability while remaining responsive for narrower Android layouts.

## Callback Wiring To Preserve

- `MapView.onToggleCameraFollow` must still call `viewModel.toggleCameraFollow()`.
- `MapView.onDisableLiveFollow` must still call `viewModel.disableLiveFollow()`.
- `MapView.onDisableReplayFollow` must still call `viewModel.toggleReplayCameraFollow()`.
- `StatusPanel.onStartStop` must still call `viewModel.stopDetection()` when running and
  `viewModel.startDetection()` when stopped.
- `StatusPanel.onToggleCamera` must still call `viewModel.toggleCameraFollow()`.
- `StatusPanel.onToggleGpsLogging` must still call `viewModel.toggleGpsLogging()`.
- `TimelineScrubber.onPlayPause` must still call `viewModel.playPause()`.
- `TimelineScrubber.onSeek` must still call `viewModel.seekTo(it)`.
- `TimelineScrubber.onSpeedChange` must still call `viewModel.setPlaybackSpeed(it)`.
- `TimelineScrubber.onToggleCameraFollow` must still call
  `viewModel.toggleReplayCameraFollow()`.
- `TimelineScrubber.allowSeek` must still be `false` for replay trace files ending in
  `.jsonl` and `true` otherwise.
- `EventToastHost` must remain overlaid outside the main `Column`.
- `ErrorSnackbar.onDismiss` must still call `viewModel.clearError()`.

## Test And Verification

Verification should include:

- Capture a before screenshot of the current Detect tab when an emulator or device is
  available. If local screenshot capture is not available, record that limitation in the
  completion notes.
- Confirm `MapView.kt` is unchanged.
- Run focused Android unit tests for detection UI components where available.
- Run Android compile or unit test verification for the app.
- Inspect the final diff for behavior-only regressions, especially callback wiring and replay seek rules.
- Capture or inspect the final Detect tab after implementation when an emulator or device is
  available, and compare it against the before screenshot.

## Success Criteria

- Detect tab looks and operates like a modern clean dashboard.
- Current stop, stop state, GPS readiness, speed, and Start/Stop action are immediately visible.
- Existing detection and replay behavior is unchanged.
- Relevant Android tests pass.
- `MapView.kt` remains untouched.
