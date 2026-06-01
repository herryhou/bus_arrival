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

// Color tokens matching DESIGN_SYSTEM.md (lighter dark theme)
val Bg0 = Color(0xFF0F0F13)
val Surface1 = Color(0xFF1A1A1E)
val Surface2 = Color(0xFF252530)
val Surface3 = Color(0xFF303038)
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
