# Bus Arrival App - Design System

Glassmorphism-based dark theme for Android tablet interface.

## Colors

### Primary Palette
| Usage | Hex | RGB | Description |
|-------|-----|-----|-------------|
| `primary` | `#6C5CE7` | Purple (108, 92, 231) | Main accent, glowing effects |
| `secondary` | `#00CEC9` | Cyan (0, 206, 201) | Active states, success, highlights |
| `error` | `#FF6B6B` | Red (255, 107, 107) | Destructive actions, errors |
| `background` | `#1A1A2E` | Deep Navy (26, 26, 46) | Base surface |
| `backgroundDark` | `#0F0F1A` | Darker Navy (15, 15, 26) | Deepest background |

### Background Gradient
```kotlin
Brush.linearGradient(
    colors = listOf(
        Color(0xFF1A1A2E).copy(alpha = 0.95f),
        Color(0xFF16213E).copy(alpha = 0.9f),
        Color(0xFF0F0F1A)
    )
)
```

### Text Alpha Levels
| State | Alpha | Usage |
|-------|-------|-------|
| `primary` | 1.0 | Headlines, titles, prominent text |
| `secondary` | 0.9 | Section headers |
| `tertiary` | 0.7 | Body text, descriptions |
| `hint` | 0.6 | Metadata, subtitles, helper text |
| `disabled` | 0.3 | Disabled states |

## Typography

### Scale
| Name | Size | Weight | Usage |
|------|------|--------|-------|
| `displaySmall` | 36sp | Bold | Page title (e.g., "Setup") |
| `headlineMedium` | 28sp | Bold | Empty state headers |
| `headlineSmall` | 24sp | Bold | Sheet headers |
| `titleLarge` | 22sp | Bold | Card titles, route names |
| `titleMedium` | 16sp | SemiBold | Section headers |
| `bodyLarge` | 16sp | Normal | Primary body text |
| `bodyMedium` | 14sp | Normal | Secondary body, metadata |
| `labelLarge` | 14sp | Normal | Button text, chip labels |
| `labelSmall` | 12sp | Medium | Small badges, indicators |

### Letter Spacing
- Display/Title: `-0.02.em` (tighter)
- Body/Label: default

## Spacing Scale
| Unit | Usage |
|------|-------|
| 4dp | Small gaps, badge spacing |
| 8dp | Icon-text gaps, list item padding |
| 12dp | Card gaps, section spacing |
| 16dp | Section gaps, card padding |
| 20dp | Card internal padding |
| 24dp | Large section gaps, sheet padding |
| 32dp | Page margins, sheet header |
| 100dp | Bottom scroll padding |

## Shapes
| Name | Size | Usage |
|------|------|-------|
| `circle` | 50% | Indicators, avatar placeholders |
| `small` | 4dp | Handle bars, small pills |
| `medium` | 16dp | Cards, buttons |
| `large` | 28dp | Bottom sheets (top corners) |

## Components

### GlassCard
Glassmorphic card with backdrop blur.

```kotlin
@Composable
fun GlassCard(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit
) {
    val cardModifier = modifier
        .fillMaxWidth()
        .clip(RoundedCornerShape(16.dp))
        .background(
            Brush.verticalGradient(
                colors = listOf(
                    Color.White.copy(alpha = 0.08f),
                    Color.White.copy(alpha = 0.03f)
                )
            )
        )
        .then(
            if (onClick != null) {
                Modifier.pointerInput(Unit) {
                    detectTapGestures { onClick() }
                }
            } else {
                Modifier
            }
        )
    
    Column(cardModifier.padding(20.dp), content)
}
```

### GlowingButton
Primary CTA with gradient glow effect.

```kotlin
@Composable
fun GlowingButton(
    onClick: () -> Unit,
    text: String,
    icon: ImageVector
) {
    Button(
        onClick = onClick,
        modifier = Modifier
            .fillMaxWidth()
            .height(56.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = Color(0xFF6C5CE7)
        ),
        shape = RoundedCornerShape(16.dp)
    ) {
        Icon(imageVector = icon, contentDescription = null)
        Spacer(modifier = Modifier.width(8.dp))
        Text(text, fontWeight = FontWeight.Bold)
    }
}
```

### ParameterSlider
Labeled slider for numeric values.

```kotlin
@Composable
fun ParameterSlider(
    label: String,
    value: Int,
    onValueChange: (Int) -> Unit,
    valueRange: IntRange = 0..100,
    suffix: String = ""
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.bodyLarge,
                color = Color.White,
                fontWeight = FontWeight.Medium
            )
            Text(
                text = "$value$suffix",
                style = MaterialTheme.typography.bodyLarge,
                color = Color(0xFF00CEC9),
                fontWeight = FontWeight.Bold
            )
        }
        Slider(
            value = value.toFloat(),
            onValueChange = { onValueChange(it.toInt()) },
            valueRange = valueRange.first.toFloat()..valueRange.last.toFloat(),
            colors = SliderDefaults.colors(
                activeTrackColor = Color(0xFF6C5CE7),
                inactiveTrackColor = Color.White.copy(alpha = 0.1f),
                thumbColor = Color(0xFF6C5CE7)
            )
        )
    }
}
```

### RouteCard
List item for route selection.

```kotlin
@Composable
fun RouteCard(
    metadata: RouteMetadata,
    isActive: Boolean,
    onClick: () -> Unit,
    onDelete: () -> Unit
) {
    GlassCard(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp),
        onClick = onClick
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(20.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = metadata.name,
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold,
                    color = if (isActive) Color(0xFF00CEC9) else Color.White
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = "${metadata.stopCount} stops",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color.White.copy(alpha = 0.6f)
                )
            }
            IconButton(onClick = onDelete) {
                Icon(
                    imageVector = Icons.Rounded.DeleteOutline,
                    contentDescription = "Delete",
                    tint = Color.White.copy(alpha = 0.5f)
                )
            }
        }
    }
}
```

## Visual Effects

### Glowing Orb Background
Ambient glow for depth.

```kotlin
Box(modifier = Modifier.fillMaxSize().blur(100.dp)) {
    Box(
        modifier = Modifier
            .size(300.dp)
            .background(
                Brush.radialGradient(
                    colors = listOf(
                        Color(0xFF6C5CE7).copy(alpha = 0.4f),
                        Color.Transparent
                    )
                )
            )
            .align(Alignment.TopStart)
            .offset(x = (-80).dp, y = (-100).dp)
    )
    Box(
        modifier = Modifier
            .size(250.dp)
            .background(
                Brush.radialGradient(
                    colors = listOf(
                        Color(0xFF00CEC9).copy(alpha = 0.3f),
                        Color.Transparent
                    )
                )
            )
            .align(Alignment.BottomEnd)
            .offset(x = 80.dp, y = 100.dp)
    )
}
```

### Active Indicator
Pulsing dot for live status.

```kotlin
Row(verticalAlignment = Alignment.CenterVertically) {
    Text(
        text = "Active",
        style = MaterialTheme.typography.labelSmall,
        fontWeight = FontWeight.Medium,
        color = Color(0xFF00CEC9)
    )
    Spacer(modifier = Modifier.width(8.dp))
    Box(
        modifier = Modifier
            .size(6.dp)
            .clip(CircleShape)
            .background(Color(0xFF00CEC9))
    )
}
```

## Bottom Sheet

### ModalBottomSheet
Glassmorphic bottom sheet with gradient.

```kotlin
ModalBottomSheet(
    onDismissRequest = { /* ... */ },
    sheetState = sheetState,
    containerColor = Color.Transparent,
    scrimColor = Color.Black.copy(alpha = 0.4f),
    dragHandle = null
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                Brush.verticalGradient(
                    colors = listOf(
                        Color(0xFF1A1A2E).copy(alpha = 0.95f),
                        Color(0xFF0F0F1A)
                    )
                ),
                shape = RoundedCornerShape(28.dp, 28.dp, 0.dp, 0.dp)
            )
            .padding(24.dp)
    ) {
        // Content
        Box(
            modifier = Modifier
                .width(40.dp)
                .height(4.dp)
                .clip(RoundedCornerShape(2.dp))
                .background(Color.White.copy(alpha = 0.3f))
                .align(Alignment.CenterHorizontally)
        )
    }
}
```

## Dialogs

### AlertDialog
Dark-themed alert dialog.

```kotlin
AlertDialog(
    onDismissRequest = onDismiss,
    containerColor = Color(0xFF1A1A2E),
    title = {
        Text(
            text = "Delete Route?",
            fontWeight = FontWeight.Bold,
            color = Color.White
        )
    },
    text = {
        Text(
            "Are you sure you want to delete \"$routeName\"? This action cannot be undone.",
            color = Color.White.copy(alpha = 0.7f)
        )
    },
    confirmButton = {
        TextButton(
            onClick = onConfirm,
            colors = ButtonDefaults.textButtonColors(
                contentColor = Color(0xFFFF6B6B)
            )
        ) {
            Text("Delete", fontWeight = FontWeight.Bold)
        }
    },
    dismissButton = {
        TextButton(onClick = onDismiss) {
            Text("Cancel", color = Color.White.copy(alpha = 0.7f))
        }
    }
)
```

## Loading States

### Page Loading
Spinner with glow effect.

```kotlin
Column(
    horizontalAlignment = Alignment.CenterHorizontally,
    verticalArrangement = Arrangement.spacedBy(16.dp)
) {
    Box(
        modifier = Modifier
            .size(60.dp)
            .blur(20.dp)
            .background(
                Brush.radialGradient(
                    colors = listOf(
                        Color(0xFF6C5CE7).copy(alpha = 0.6f),
                        Color.Transparent
                    )
                )
            )
    )
    CircularProgressIndicator(
        modifier = Modifier.size(48.dp),
        color = Color(0xFF6C5CE7),
        strokeWidth = 3.dp
    )
    Text(
        text = "Loading routes...",
        style = MaterialTheme.typography.bodyLarge,
        color = Color.White.copy(alpha = 0.7f)
    )
}
```

### Empty State
Animated icon with call-to-action.

```kotlin
Column(
    horizontalAlignment = Alignment.CenterHorizontally
) {
    val infiniteTransition = rememberInfiniteTransition(label = "pulse")
    val scale by infiniteTransition.animateFloat(
        initialValue = 1f,
        targetValue = 1.1f,
        animationSpec = infiniteRepeatable(
            animation = tween(1500, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "pulse"
    )

    Icon(
        imageVector = Icons.Rounded.Add,
        modifier = Modifier.size(80.dp).scale(scale),
        tint = Color(0xFF6C5CE7).copy(alpha = 0.8f)
    )

    Spacer(modifier = Modifier.height(24.dp))

    Text(
        text = "No routes yet",
        style = MaterialTheme.typography.headlineMedium,
        fontWeight = FontWeight.Bold,
        color = Color.White
    )

    GlowingButton(onClick = onAddRoute, text = "Add Route", icon = Icons.Rounded.Add)
}
```

## Iconography

### Icons
| Name | Material Icon | Usage |
|------|----------------|-------|
| `Add` | `Icons.Rounded.Add` | Add new item |
| `Close` | `Icons.Rounded.Close` | Dismiss sheets/dialogs |
| `DeleteOutline` | `Icons.Rounded.DeleteOutline` | Delete actions |
| `Tune` | `Icons.Rounded.Tune` | Settings/parameters |

### Sizes
| Context | Size |
|---------|------|
| Inline with text | 20-24dp |
| Card icons | 28dp |
| Buttons | 24dp |
| Empty states | 80dp |
| Loading glow | 60dp |

## Safe Areas

Always respect system insets:

```kotlin
LazyColumn(
    modifier = Modifier.fillMaxSize(),
    contentPadding = WindowInsets.safeDrawing
        .only(WindowInsetsSides.Horizontal)
        .asPaddingValues()
) {
    // content
}
```

## Usage Principles

1. **Depth through blur**: Use `.blur(100.dp)` for ambient glows
2. **Alpha hierarchy**: Text importance through 1.0 → 0.3 alpha scale
3. **Glow sparingly**: Purple for primary, cyan for active/success
4. **Glass cards**: 0.08 → 0.03 vertical gradient, white alpha
5. **Round corners**: 16dp for cards, 28dp for sheets
6. **Spacing rhythm**: Multiples of 4dp (8, 12, 16, 24, 32)
