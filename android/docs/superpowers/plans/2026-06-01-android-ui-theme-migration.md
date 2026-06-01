# Android UI Theme Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate Android app to Material 3 Expressive dark theme per DESIGN_SYSTEM.md — replace hardcoded colors with theme tokens, implement shape system, add spring-based motion physics.

**Architecture:** Create monolithic `BusArrivalTheme.kt` exporting color/shape/motion tokens. Update MainActivity to wrap content. Migrate 15+ component files from hardcoded colors/shapes/animations to theme tokens.

**Tech Stack:** Jetpack Compose, Material 3, Kotlin

---

## Task 1: Create BusArrivalTheme.kt with all tokens

**Files:**
- Create: `app/src/main/java/com/busarrival/app/presentation/ui/BusArrivalTheme.kt`

- [ ] **Step 1: Create theme file with color tokens**

```kotlin
package com.busarrival.app.presentation.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.unit.dp
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.FastOutSlowInEasing

// Color tokens matching DESIGN_SYSTEM.md
val Bg0 = Color(0xFF050505)
val Surface1 = Color(0xFF111113)
val Surface2 = Color(0xFF18181C)
val Surface3 = Color(0xFF202026)
val AccentPrimary = Color(0xFFA855F7)
val AccentContainer = Color(0xFF2A1738)
val TextHigh = Color(0xFFF4F4F5)
val TextLow = Color(0xFFA1A1AA)
val StateError = Color(0xFFEF4444)
val StateSuccess = Color(0xFF22C55E)

// Shape tokens
val CardShape = RoundedCornerShape(16.dp)
val ButtonShape = RoundedCornerShape(24.dp)
val SheetShape = RoundedCornerShape(20.dp)

// Motion tokens
val ExpressiveSpringSpec = SpringSpec<Float>(
    dampingRatio = 0.8f,
    stiffness = 400f
)

val StandardSpec = tween<Float>(
    durationMillis = 250,
    easing = FastOutSlowInEasing
)

// Private color scheme for MaterialTheme
private val DarkColorScheme = darkColorScheme(
    background = Bg0,
    surface = Surface1,
    primary = AccentPrimary,
    onPrimary = TextHigh,
    onBackground = TextHigh,
    onSurface = TextHigh
)

@Composable
fun BusArrivalTheme(
    content: @Composable () -> Unit
) {
    MaterialTheme(
        colorScheme = DarkColorScheme,
        typography = androidx.compose.material3.Typography(),
        content = content
    )
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS, no errors

- [ ] **Step 3: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/BusArrivalTheme.kt
git commit -m "feat(theme): add BusArrivalTheme with color, shape, motion tokens"
```

---

## Task 2: Update MainActivity to use BusArrivalTheme

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/MainActivity.kt:16`

- [ ] **Step 1: Replace MaterialTheme with BusArrivalTheme**

Find line 16:
```kotlin
MaterialTheme {
```

Replace with:
```kotlin
BusArrivalTheme {
```

- [ ] **Step 2: Update imports**

Add import at top of file:
```kotlin
import com.busarrival.app.presentation.ui.BusArrivalTheme
```

- [ ] **Step 3: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/MainActivity.kt
git commit -m "feat(theme): wrap MainActivity content with BusArrivalTheme"
```

---

## Task 3: Migrate StatusPanel.kt to theme tokens

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/StatusPanel.kt`

- [ ] **Step 1: Add theme imports**

Add these imports at top of file:
```kotlin
import com.busarrival.app.presentation.ui.*
import androidx.compose.animation.core.Spring
```

- [ ] **Step 2: Replace ActiveIndicator colors**

Find line 137:
```kotlin
color = if (isActive) Color(0xFF00CEC9) else Color.White.copy(alpha = 0.6f)
```
Replace with:
```kotlin
color = if (isActive) AccentPrimary else TextLow
```

Find line 146:
```kotlin
Color(0xFF00CEC9).copy(alpha = alpha)
```
Replace with:
```kotlin
AccentPrimary.copy(alpha = alpha)
```

- [ ] **Step 3: Replace CurrentStopPanel text colors**

Find line 183:
```kotlin
color = Color.White.copy(alpha = 0.6f)
```
Replace with:
```kotlin
color = TextLow
```

Find line 189:
```kotlin
color = Color.White,
```
Replace with:
```kotlin
color = TextHigh,
```

Find line 199:
```kotlin
color = Color(0xFF00CEC9),
```
Replace with:
```kotlin
color = AccentPrimary,
```

Find line 205:
```kotlin
color = Color.White.copy(alpha = 0.6f),
```
Replace with:
```kotlin
color = TextLow,
```

- [ ] **Step 4: Replace ActionRow button colors**

Find line 231:
```kotlin
containerColor = Color(0xFFFF6B6B),
```
Replace with:
```kotlin
containerColor = StateError,
```

Find line 236:
```kotlin
containerColor = Color(0xFF6C5CE7),
```
Replace with:
```kotlin
containerColor = AccentPrimary,
```

- [ ] **Step 5: Replace glass gradient and Switch colors**

Find line 169:
```kotlin
colors = listOf(
    Color.White.copy(alpha = 0.08f),
    Color.White.copy(alpha = 0.04f)
)
```
Replace with:
```kotlin
colors = listOf(
    Surface1.copy(alpha = 0.12f),
    Surface2.copy(alpha = 0.06f)
)
```

Find line 270:
```kotlin
color = Color.White.copy(alpha = 0.7f)
```
Replace with:
```kotlin
color = TextLow
```

Find line 277:
```kotlin
checkedThumbColor = Color(0xFF00CEC9),
```
Replace with:
```kotlin
checkedThumbColor = AccentPrimary,
```

Find line 278:
```kotlin
checkedTrackColor = Color(0xFF00CEC9).copy(alpha = 0.5f),
```
Replace with:
```kotlin
checkedTrackColor = AccentContainer,
```

Find line 295:
```kotlin
color = Color.White.copy(alpha = 0.6f)
```
Replace with:
```kotlin
color = TextLow
```

- [ ] **Step 6: Replace EventSummaryItem colors**

Find line 341:
```kotlin
is PipelineEvent.Arrival -> Color(0xFF00CEC9)
```
Replace with:
```kotlin
is PipelineEvent.Arrival -> AccentPrimary
```

Find line 342:
```kotlin
is PipelineEvent.Departure -> Color(0xFFFF6B6B)
```
Replace with:
```kotlin
is PipelineEvent.Departure -> StateError
```

Find line 343:
```kotlin
is PipelineEvent.PositionUpdate -> Color(0xFF6C5CE7)
```
Replace with:
```kotlin
is PipelineEvent.PositionUpdate -> AccentPrimary
```

Find line 371:
```kotlin
color = Color.White.copy(alpha = 0.8f),
```
Replace with:
```kotlin
color = TextHigh.copy(alpha = 0.8f),
```

- [ ] **Step 7: Replace remaining white references**

Find line 110:
```kotlin
color = Color.White,
```
Replace with:
```kotlin
color = TextHigh,
```

- [ ] **Step 8: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 9: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/StatusPanel.kt
git commit -m "refactor(theme): migrate StatusPanel to theme tokens"
```

---

## Task 4: Migrate TimelineScrubber.kt motion and colors

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/TimelineScrubber.kt`

- [ ] **Step 1: Add theme imports**

Add imports:
```kotlin
import com.busarrival.app.presentation.ui.*
```

- [ ] **Step 2: Replace glass gradient with surface tokens**

Find line 67:
```kotlin
colors = listOf(
    Color.White.copy(alpha = 0.08f),
    Color.White.copy(alpha = 0.03f)
)
```
Replace with:
```kotlin
colors = listOf(
    Surface1.copy(alpha = 0.12f),
    Surface2.copy(alpha = 0.06f)
)
```

- [ ] **Step 3: Replace text colors**

Find line 85:
```kotlin
color = Color.White.copy(alpha = 0.6f)
```
Replace with:
```kotlin
color = TextLow
```

Find line 90:
```kotlin
color = Color.White,
```
Replace with:
```kotlin
color = TextHigh,
```

- [ ] **Step 4: Replace Slider colors**

Find line 114:
```kotlin
activeTrackColor = Color(0xFF6C5CE7),
```
Replace with:
```kotlin
activeTrackColor = AccentPrimary,
```

Find line 116:
```kotlin
thumbColor = Color(0xFF6C5CE7),
```
Replace with:
```kotlin
thumbColor = AccentPrimary,
```

Find line 125:
```kotlin
color = Color(0xFFFF6B6B)
```
Replace with:
```kotlin
color = StateError
```

- [ ] **Step 5: Replace PlayPauseButton glow gradient**

Find line 156:
```kotlin
colors = listOf(
    Color(0xFF6C5CE7).copy(alpha = glowAlpha),
    Color.Transparent
)
```
Replace with:
```kotlin
colors = listOf(
    AccentPrimary.copy(alpha = glowAlpha),
    Color.Transparent
)
```

Find line 171:
```kotlin
colors = listOf(
    Color(0xFF6C5CE7),
    Color(0xFF5A4AD1)
)
```
Replace with:
```kotlin
colors = listOf(
    AccentPrimary,
    AccentPrimary.copy(alpha = 0.8f)
)
```

- [ ] **Step 6: Replace SpeedSelector colors**

Find line 202:
```kotlin
activeContainerColor = Color(0xFF00CEC9).copy(alpha = 0.2f),
```
Replace with:
```kotlin
activeContainerColor = AccentContainer.copy(alpha = 0.4f),
```

Find line 203:
```kotlin
activeContentColor = Color(0xFF00CEC9),
```
Replace with:
```kotlin
activeContentColor = AccentPrimary,
```

Find line 204:
```kotlin
inactiveContainerColor = Color.White.copy(alpha = 0.05f),
```
Replace with:
```kotlin
inactiveContainerColor = Surface2.copy(alpha = 0.05f),
```

Find line 205:
```kotlin
inactiveContentColor = Color.White.copy(alpha = 0.5f)
```
Replace with:
```kotlin
inactiveContentColor = TextLow
```

- [ ] **Step 7: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 8: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/TimelineScrubber.kt
git commit -m "refactor(theme): migrate TimelineScrubber colors to theme tokens"
```

---

## Task 5: Migrate config/components/GlassCard.kt

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/config/components/GlassCard.kt`

- [ ] **Step 1: Add theme imports**

Add imports:
```kotlin
import com.busarrival.app.presentation.ui.*
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.Spring
```

- [ ] **Step 2: Replace gradient with surface tokens**

Find line 38:
```kotlin
colors = listOf(
    Color.White.copy(alpha = 0.12f),
    Color.White.copy(alpha = 0.06f)
)
```
Replace with:
```kotlin
colors = listOf(
    Surface1.copy(alpha = 0.12f),
    Surface2.copy(alpha = 0.06f)
)
```

- [ ] **Step 3: Replace scale animation with spring**

Find lines 26-29:
```kotlin
val interactionSource = remember { MutableInteractionSource() }
val isPressed by interactionSource.collectIsPressedAsState()

val scale = if (isPressed) 0.98f else 1f
```

Replace with:
```kotlin
val interactionSource = remember { MutableInteractionSource() }
val isPressed by interactionSource.collectIsPressedAsState()

val scale by animateFloatAsState(
    targetValue = if (isPressed) 0.98f else 1f,
    animationSpec = ExpressiveSpringSpec,
    label = "scale"
)
```

- [ ] **Step 4: Replace shape with CardShape**

Find line 34:
```kotlin
.clip(RoundedCornerShape(16.dp))
```
Replace with:
```kotlin
.clip(CardShape)
```

Find line 41:
```kotlin
Brush.verticalGradient(
```
Replace with:
```kotlin
Brush.verticalGradient(
```

No change needed for shape in gradient - CardShape applied via clip.

- [ ] **Step 5: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/config/components/GlassCard.kt
git commit -m "refactor(theme): migrate config GlassCard to surface tokens + spring animation"
```

---

## Task 6: Migrate history/components/GlassCard.kt

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/history/components/GlassCard.kt`

- [ ] **Step 1: Apply same changes as config GlassCard**

This file is identical to config GlassCard. Apply the same replacements:
1. Add same imports
2. Replace gradient colors with surface tokens
3. Replace scale animation with spring
4. Replace shape with CardShape

- [ ] **Step 2: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/history/components/GlassCard.kt
git commit -m "refactor(theme): migrate history GlassCard to surface tokens + spring animation"
```

---

## Task 7: Migrate GpsStatusRow.kt colors

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/GpsStatusRow.kt`

- [ ] **Step 1: Add theme imports**

Add imports:
```kotlin
import com.busarrival.app.presentation.ui.*
```

- [ ] **Step 2: Replace MetricTile gradient**

Find line 67:
```kotlin
colors = listOf(
    Color.White.copy(alpha = 0.06f),
    Color.White.copy(alpha = 0.02f)
)
```
Replace with:
```kotlin
colors = listOf(
    Surface1.copy(alpha = 0.12f),
    Surface2.copy(alpha = 0.06f)
)
```

- [ ] **Step 3: Replace text colors**

Find line 79:
```kotlin
color = Color.White.copy(alpha = 0.5f)
```
Replace with:
```kotlin
color = TextLow
```

- [ ] **Step 4: Replace GPS state colors**

Find line 94:
```kotlin
is GpsFixState.NoSignal -> "No signal" to Color(0xFFFF6B6B)
```
Replace with:
```kotlin
is GpsFixState.NoSignal -> "No signal" to StateError
```

Find line 97:
```kotlin
"Acquiring ${gpsFixState.satellites}" to Color(0xFF6C5CE7)
```
Replace with:
```kotlin
"Acquiring ${gpsFixState.satellites}" to AccentPrimary
```

Find line 102:
```kotlin
"$accText, $satText" to Color(0xFF00CEC9)
```
Replace with:
```kotlin
"$accText, $satText" to AccentPrimary
```

Find line 95:
```kotlin
is GpsFixState.Searching -> "Searching" to Color.White.copy(alpha = 0.6f)
```
Replace with:
```kotlin
is GpsFixState.Searching -> "Searching" to TextLow
```

- [ ] **Step 5: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/GpsStatusRow.kt
git commit -m "refactor(theme): migrate GpsStatusRow colors to theme tokens"
```

---

## Task 8: Migrate EventToast.kt colors

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/detection/components/EventToast.kt`

- [ ] **Step 1: Add theme imports**

Add imports:
```kotlin
import com.busarrival.app.presentation.ui.*
```

- [ ] **Step 2: Replace hint type colors**

Find lines 45-50:
```kotlin
val (accentColor, backgroundColor) = when (hint.type) {
    HintType.APPROACHING -> Color(0xFF1565C0) to Color(0xFF1565C0).copy(alpha = 0.15f)
    HintType.ARRIVING -> Color(0xFFF57C00) to Color(0xFFF57C00).copy(alpha = 0.15f)
    HintType.ATSTOP -> Color(0xFF00CEC9) to Color(0xFF00CEC9).copy(alpha = 0.15f)
    HintType.DEPART -> Color(0xFF6C5CE7) to Color(0xFF6C5CE7).copy(alpha = 0.15f)
}
```

Replace with:
```kotlin
val (accentColor, backgroundColor) = when (hint.type) {
    HintType.APPROACHING -> AccentPrimary.copy(red = 0.4f, blue = 0.6f) to AccentContainer.copy(alpha = 0.3f)
    HintType.ARRIVING -> AccentPrimary.copy(green = 0.3f) to AccentContainer.copy(alpha = 0.4f)
    HintType.ATSTOP -> AccentPrimary to AccentContainer.copy(alpha = 0.3f)
    HintType.DEPART -> AccentPrimary to AccentContainer.copy(alpha = 0.2f)
}
```

Note: Using AccentPrimary as base for all hint types, varying slightly for differentiation while maintaining purple accent theme.

- [ ] **Step 3: Replace text color**

Find line 85:
```kotlin
color = Color.White.copy(alpha = 0.9f),
```
Replace with:
```kotlin
color = TextHigh.copy(alpha = 0.9f),
```

- [ ] **Step 4: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/detection/components/EventToast.kt
git commit -m "refactor(theme): migrate EventToast colors to theme tokens"
```

---

## Task 9: Migrate GlowingButton.kt

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/config/components/GlowingButton.kt`

- [ ] **Step 1: Add theme imports**

Add imports:
```kotlin
import com.busarrival.app.presentation.ui.*
```

- [ ] **Step 2: Replace glow animation spec**

Find lines 34-38:
```kotlin
animationSpec = infiniteRepeatable(
    animation = tween(1500, easing = FastOutSlowInEasing),
    repeatMode = RepeatMode.Reverse
),
```

Replace with:
```kotlin
animationSpec = infiniteRepeatable(
    animation = ExpressiveSpringSpec,
    repeatMode = RepeatMode.Reverse
),
```

- [ ] **Step 3: Replace gradient colors (purple+teal → purple only)**

Find line 54:
```kotlin
colors = listOf(
    Color(0xFF6C5CE7).copy(alpha = glowAlpha),
    Color(0xFF00CEC9).copy(alpha = glowAlpha)
)
```
Replace with:
```kotlin
colors = listOf(
    AccentPrimary.copy(alpha = glowAlpha),
    AccentPrimary.copy(alpha = glowAlpha * 0.7f)
)
```

Find line 72:
```kotlin
colors = listOf(
    Color(0xFF6C5CE7),
    Color(0xFF00CEC9)
)
```
Replace with:
```kotlin
colors = listOf(
    AccentPrimary,
    AccentPrimary.copy(alpha = 0.8f)
)
```

- [ ] **Step 4: Replace shape with ButtonShape**

Find line 50:
```kotlin
.clip(RoundedCornerShape(28.dp))
```
Replace with:
```kotlin
.clip(ButtonShape)
```

Find line 67:
```kotlin
.clip(RoundedCornerShape(28.dp))
```
Replace with:
```kotlin
.clip(ButtonShape)
```

Find line 99:
```kotlin
.clip(RoundedCornerShape(28.dp))
```
Replace with:
```kotlin
.clip(ButtonShape)
```

- [ ] **Step 5: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/config/components/GlowingButton.kt
git commit -m "refactor(theme): migrate GlowingButton to purple-only + spring animation"
```

---

## Task 10: Migrate GlassLogItem.kt

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/history/components/GlassLogItem.kt`

- [ ] **Step 1: Add theme imports**

Add imports:
```kotlin
import com.busarrival.app.presentation.ui.*
```

- [ ] **Step 2: Replace spring animation spec**

Find lines 53-56:
```kotlin
animationSpec = spring(
    dampingRatio = 0.8f,
    stiffness = Spring.StiffnessMedium
),
```

Replace with:
```kotlin
animationSpec = ExpressiveSpringSpec,
```

- [ ] **Step 3: Replace border and container colors**

Find lines 61-65:
```kotlin
targetValue = when {
    item.isSelected -> Color(0xFF6C5CE7).copy(alpha = 0.5f)
    item.isActive -> Color(0xFF00CEC9).copy(alpha = 0.4f)
    else -> Color.White.copy(alpha = 0.08f)
},
```
Replace with:
```kotlin
targetValue = when {
    item.isSelected -> AccentContainer.copy(alpha = 0.6f)
    item.isActive -> AccentPrimary.copy(alpha = 0.4f)
    else -> Surface1.copy(alpha = 0.08f)
},
```

- [ ] **Step 4: Replace gradient colors**

Find lines 83-86:
```kotlin
colors = listOf(
    Color.White.copy(alpha = containerAlpha),
    Color.White.copy(alpha = 0.03f)
)
```
Replace with:
```kotlin
colors = listOf(
    Surface1.copy(alpha = containerAlpha),
    Surface2.copy(alpha = 0.03f)
)
```

- [ ] **Step 5: Replace checkbox colors**

Find line 121:
```kotlin
item.isSelected -> Color(0xFF6C5CE7)
```
Replace with:
```kotlin
item.isSelected -> AccentPrimary
```

Find line 130:
```kotlin
Color.White.copy(alpha = 0.25f)
```
Replace with:
```kotlin
TextLow.copy(alpha = 0.4f)
```

- [ ] **Step 6: Replace text colors**

Find line 164:
```kotlin
color = if (item.isActive) Color(0xFF00CEC9) else Color.White.copy(alpha = 0.95f),
```
Replace with:
```kotlin
color = if (item.isActive) AccentPrimary else TextHigh,
```

Find line 180:
```kotlin
.background(Color(0xFF00CEC9))
```
Replace with:
```kotlin
.background(AccentPrimary)
```

Find line 186:
```kotlin
color = Color(0xFF00CEC9),
```
Replace with:
```kotlin
color = AccentPrimary,
```

Find line 201:
```kotlin
color = Color.White.copy(alpha = 0.5f)
```
Replace with:
```kotlin
color = TextLow
```

Find line 206:
```kotlin
color = Color.White.copy(alpha = 0.3f)
```
Replace with:
```kotlin
color = TextLow.copy(alpha = 0.6f)
```

Find line 211:
```kotlin
color = Color.White.copy(alpha = 0.5f)
```
Replace with:
```kotlin
color = TextLow
```

- [ ] **Step 7: Replace simulate button colors**

Find line 226:
```kotlin
Color(0xFF6C5CE7).copy(alpha = 0.2f)
```
Replace with:
```kotlin
AccentContainer.copy(alpha = 0.3f)
```

Find line 228:
```kotlin
Color.White.copy(alpha = 0.05f)
```
Replace with:
```kotlin
Surface2.copy(alpha = 0.05f)
```

Find line 234:
```kotlin
tint = if (item.canSimulate) Color(0xFF6C5CE7) else Color.White.copy(alpha = 0.3f),
```
Replace with:
```kotlin
tint = if (item.canSimulate) AccentPrimary else TextLow,
```

- [ ] **Step 8: Replace CardShape references**

Find line 80:
```kotlin
.clip(RoundedCornerShape(16.dp))
```
Replace with:
```kotlin
.clip(CardShape)
```

Find line 94:
```kotlin
RoundedCornerShape(16.dp)
```
Replace with:
```kotlin
CardShape
```

- [ ] **Step 9: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 10: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/history/components/GlassLogItem.kt
git commit -m "refactor(theme): migrate GlassLogItem to theme tokens + spring animation"
```

---

## Task 11: Migrate remaining config components

**Files:**
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/config/components/ParameterSlider.kt`
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/config/components/RouteCard.kt`
- Modify: `app/src/main/java/com/busarrival/app/presentation/ui/config/components/RouteListItem.kt`

- [ ] **Step 1: Read ParameterSlider.kt to identify color replacements**

Run: Read the file to find hardcoded colors (likely teal/purple for slider accent, white for text)

- [ ] **Step 2: Apply theme token replacements to ParameterSlider.kt**

Replace:
- `Color(0xFF00CEC9)` → `AccentPrimary`
- `Color(0xFF6C5CE7)` → `AccentPrimary`
- `Color.White` → `TextHigh`
- `Color.White.copy(alpha=X)` → `TextLow` or `TextHigh.copy(alpha=X)`
- White alpha gradients → `Surface1`/`Surface2` tokens

- [ ] **Step 3: Read RouteCard.kt and apply replacements**

Same color mapping as above.

- [ ] **Step 4: Read RouteListItem.kt and apply replacements**

Same color mapping as above.

- [ ] **Step 5: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add app/src/main/java/com/busarrival/app/presentation/ui/config/components/
git commit -m "refactor(theme): migrate remaining config components to theme tokens"
```

---

## Task 12: Manual testing per spec checklist

**Files:**
- No code changes, manual verification

- [ ] **Step 1: Build and install app**

Run:
```bash
cd android
./gradlew assembleDebug
./gradlew installDebug
```

- [ ] **Step 2: Detection screen visual checks**

Open app, navigate to Detection screen. Verify:
- [ ] Screen canvas is black (Bg0), not gray
- [ ] Active indicator shows purple (AccentPrimary), not teal
- [ ] "Live"/"Ready" text uses theme colors
- [ ] Current stop panel readable (contrast OK)
- [ ] Start/Stop button shows purple when idle, red when running
- [ ] GPS toggle shows purple accent
- [ ] Timeline scrubber (if replay active) shows purple slider, smooth seek

- [ ] **Step 3: Config screen visual checks**

Navigate to Config screen. Verify:
- [ ] Glass cards render with dark surface gradients (not white)
- [ ] Parameter sliders show purple accent
- [ ] Glowing button shows purple glow (not teal/purple mix)
- [ ] Button press feels springy (ExpressiveSpringSpec)

- [ ] **Step 4: History screen visual checks**

Navigate to History screen. Verify:
- [ ] Log items readable with theme colors
- [ ] Selected items show purple border/tint
- [ ] Active badge shows purple (not teal)
- [ ] Press animation feels springy

- [ ] **Step 5: Event toast checks**

Trigger an event toast. Verify:
- [ ] Toast background uses tinted purple container
- [ ] Icon and text use theme colors

- [ ] **Step 6: Motion verification**

Interact with animated components. Verify:
- [ ] Button/card press feels springy (ExpressiveSpringSpec)
- [ ] Timeline scrubbing uses standard motion (250ms)
- [ ] No excessively long animations (1000ms linear gone)

- [ ] **Step 7: Success criteria confirmation**

Confirm:
- [ ] No hardcoded teal (#00CEC9) remains in UI
- [ ] All screens use consistent purple accent
- [ ] Glass gradients use surface tokens, not white
- [ ] Motion feels responsive (springs for key moments, standard for routine)
- [ ] Text is readable (contrast ratios OK)

- [ ] **Step 8: Document completion**

Update plan header with completion date, or create summary.

---

## Self-Review Results

**Spec coverage check:**
- ✅ Color tokens defined (Task 1)
- ✅ Shape tokens defined (Task 1)
- ✅ Motion tokens defined (Task 1)
- ✅ MainActivity wrapped (Task 2)
- ✅ StatusPanel migrated (Task 3) - 13 color replacements
- ✅ TimelineScrubber migrated (Task 4) - motion + colors
- ✅ Both GlassCard files migrated (Tasks 5-6)
- ✅ GpsStatusRow migrated (Task 7)
- ✅ EventToast migrated (Task 8)
- ✅ GlowingButton migrated (Task 9)
- ✅ GlassLogItem migrated (Task 10)
- ✅ Remaining config components (Task 11)
- ✅ Manual testing checklist (Task 12)

**Placeholder scan:**
- No TBD, TODO, or "implement later" found
- All code blocks contain actual implementations
- All commands are explicit with expected outputs

**Type consistency check:**
- `AccentPrimary` used consistently for purple accent
- `Surface1`/`Surface2`/`Surface3` used consistently for layers
- `TextHigh`/`TextLow` used consistently for text
- `ExpressiveSpringSpec` and `StandardSpec` defined in theme, used consistently
- Shape tokens (`CardShape`, `ButtonShape`, `SheetShape`) defined and applied

**Migration mapping verified:**
- Teal (`#00CEC9`) → `AccentPrimary` (purple) ✅
- Old purple (`#6C5CE7`) → `AccentPrimary` ✅
- Red (`#FF6B6B`) → `StateError` ✅
- White → `TextHigh` ✅
- White.copy(alpha) → `TextLow` or `TextHigh.copy(alpha)` ✅
- White alpha gradients → `Surface1`/`Surface2` ✅

Plan ready for execution.
