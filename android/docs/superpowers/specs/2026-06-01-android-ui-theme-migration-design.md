# Android UI Theme Migration Design

**Date:** 2026-06-01
**Scope:** Full Material 3 Expressive dark theme migration

## Overview

Migrate Android app to Material 3 Expressive dark theme per `DESIGN_SYSTEM.md`. Replace hardcoded colors with theme tokens, implement shape system, add spring-based motion physics.

## Architecture

Create monolithic theme file: `ui/BusArrivalTheme.kt`

```
ui/
├── BusArrivalTheme.kt (NEW - all tokens)
├── detection/components/ (existing)
├── config/components/ (existing)
└── history/components/ (existing)
```

**Theme exports:**
- `BusArrivalTheme` - MaterialTheme wrapper
- Color tokens: `bg0`, `surface1`, `surface2`, `surface3`, `accentPrimary`, `accentContainer`, `textHigh`, `textLow`, `stateError`, `stateSuccess`
- Shape tokens: `CardShape`, `ButtonShape`, `SheetShape`
- Motion tokens: `SpringSpec`, `ExpressiveSpec`
- Typography: MaterialTheme defaults with weight adjustments

## Color System

Design system specifies black-based canvas with purple accent.

### Tokens

```kotlin
val Bg0 = Color(0xFF050505)           // App canvas
val Surface1 = Color(0xFF111113)       // Base cards
val Surface2 = Color(0xFF18181C)       // Raised containers
val Surface3 = Color(0xFF202026)       // Focus/active state
val AccentPrimary = Color(0xFFA855F7)  // Brand actions
val AccentContainer = Color(0xFF2A1738) // Tinted containers
val TextHigh = Color(0xFFF4F4F5)       // Primary text
val TextLow = Color(0xFFA1A1AA)        // Secondary text
val StateError = Color(0xFFEF4444)     // Errors
val StateSuccess = Color(0xFF22C55E)   // Success
```

### Migration Mapping

- `Color(0xFF00CEC9)` (teal) → `AccentPrimary` (purple replaces teal)
- `Color(0xFF6C5CE7)` (old purple) → `AccentPrimary`
- `Color(0xFFFF6B6B)` (red) → `StateError`
- `Color.White` → `TextHigh`
- `Color.White.copy(alpha=0.6)` → `TextLow`

## Shapes & Elevation

Design system: rounded geometry, tonal layering over blur/glow.

### Shape Tokens

```kotlin
val CardShape = RoundedCornerShape(16.dp)    // Primary cards
val ButtonShape = RoundedCornerShape(24.dp)  // Action buttons (more rounded)
val SheetShape = RoundedCornerShape(20.dp)   // Overlays (softer than cards)
```

### Elevation Strategy

- Glass gradients: `Surface1.copy(alpha=0.12f)` → `Surface2.copy(alpha=0.06f)`
- Active states: `Surface3` tint (no bright shadows)
- Depth from tonal separation, not glow

## Motion System

Design system: spring physics for expressive moments, standard motion for routine tasks. Animations capped at 150-350ms.

### Motion Tokens

```kotlin
val ExpressiveSpringSpec = spring<Float>(
    dampingRatio = 0.8f,
    stiffness = 400f
)

val StandardSpec = tween<Float>(
    durationMillis = 250,
    easing = FastOutSlowInEasing
)
```

### Migration Targets

- `ActiveIndicator`: 1000ms linear → 250ms standard (broadcast indicator, routine)
- `GlassCard` press: `ExpressiveSpringSpec` (expressive moment)
- `TimelineScrubber`: `StandardSpec` for seek (routine navigation)
- Button interactions: `ExpressiveSpringSpec` (expressive moment)
- Card expand/collapse: `StandardSpec` (routine)

**Per design system:** Use expressive motion for moments of delight or major transitions (button presses, card interactions). Use standard motion for routine navigation and utility tasks (scrubbers, toggles).

## Component Migration

**Priority order:**

1. Create `ui/BusArrivalTheme.kt`
2. Update `MainActivity.kt` to wrap content with `BusArrivalTheme`
3. `StatusPanel.kt` - 13 color/shape replacements
4. `GpsStatusRow.kt` - GPS metric tiles (color replacements, including GPS state colors)
5. `GlassCard.kt` - BOTH implementations (`config/components/GlassCard.kt` AND `history/components/GlassCard.kt`)
6. Config components - `GlowingButton`, `ParameterSlider`, `RouteCard`
7. History components - `GlassLogItem`
8. `EventToast` - notifications
9. `MapView.kt` - canvas/overlay colors

**Files touched:**
- 1 new: `ui/BusArrivalTheme.kt`
- 1 modify: `presentation/MainActivity.kt`
- 15 component files (color/shape/motion updates):
  - `detection/components/`: StatusPanel, GpsStatusRow, TimelineScrubber, EventToast, MapView (5 files)
  - `config/components/`: GlassCard, GlowingButton, ParameterSlider, RouteCard, RouteListItem (5 files)
  - `history/components/`: GlassCard, GlassLogItem (2 files)
  - Plus 3 additional detection components (EventToastHost, VehicleHeadingMarker, MapCoordinateUtils)

## Testing Protocol

### Manual Test Checklist

**Detection Screen:**
- [ ] Screen loads with black canvas
- [ ] Purple accent on active indicator (not teal)
- [ ] All text readable (contrast check)
- [ ] Start/Stop button: purple → red press
- [ ] GPS toggle: spring animation
- [ ] Timeline scrubber: spring seek

**Config Screen:**
- [ ] Glass cards render with surface gradients
- [ ] Sliders show purple accent
- [ ] Button presses use spring easing

**History Screen:**
- [ ] Log items readable
- [ ] Shapes consistent (CardShape)
- [ ] All text uses theme colors

**Events:**
- [ ] Arrival events show purple markers
- [ ] State transitions use spring physics

### Success Criteria

- No hardcoded colors remain in UI code (all 15 component files use theme tokens)
- Animations follow motion system (expressive spring for key moments, standard for routine)
- Visual consistency across all screens
- Contrast ratios readable per WCAG AA

## Implementation Notes

- Teal (`#00CEC9`) is deprecated — purple takes all accent roles
- Glass morphism调整为 uses surface tokens, not white
- Motion applies to all transitions including broadcast indicators
- Theme is monolithic — all tokens in one file
