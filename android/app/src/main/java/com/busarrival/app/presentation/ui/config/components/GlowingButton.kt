package com.busarrival.app.presentation.ui.config.components

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.material.icons.Icons as MaterialIcons

@Composable
fun GlowingButton(
    onClick: () -> Unit,
    text: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    modifier: Modifier = Modifier
) {
    val infiniteTransition = rememberInfiniteTransition(label = "glow")
    val glowAlpha by infiniteTransition.animateFloat(
        initialValue = 0.4f,
        targetValue = 0.7f,
        animationSpec = infiniteRepeatable(
            animation = tween(1500, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "glow"
    )

    Box(
        modifier = modifier,
        contentAlignment = Alignment.Center
    ) {
        // Glow effect
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(56.dp)
                .clip(RoundedCornerShape(28.dp))
                .background(
                    brush = Brush.horizontalGradient(
                        colors = listOf(
                            Color(0xFF6C5CE7).copy(alpha = glowAlpha),
                            Color(0xFF00CEC9).copy(alpha = glowAlpha)
                        )
                    )
                )
                .scale(1.05f)
        )

        // Main button
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(56.dp)
                .clip(RoundedCornerShape(28.dp))
                .background(
                    brush = Brush.horizontalGradient(
                        colors = listOf(
                            Color(0xFF6C5CE7),
                            Color(0xFF00CEC9)
                        )
                    )
                ),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = Color.White,
                modifier = Modifier.size(24.dp)
            )
            Spacer(modifier = Modifier.width(12.dp))
            Text(
                text = text,
                color = Color.White,
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold
            )
        }

        // Clickable overlay
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(56.dp)
                .clip(RoundedCornerShape(28.dp))
                .background(Color.Transparent)
        ) {
            androidx.compose.material3.Button(
                onClick = onClick,
                modifier = Modifier.fillMaxSize(),
                colors = androidx.compose.material3.ButtonDefaults.buttonColors(
                    containerColor = Color.Transparent,
                    contentColor = Color.Transparent
                ),
                elevation = androidx.compose.material3.ButtonDefaults.buttonElevation(0.dp),
                content = {}
            )
        }
    }
}
