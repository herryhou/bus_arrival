package com.busarrival.app.presentation.navigation.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

data class TabItem(
    val route: String,
    val title: String,
    val icon: ImageVector,
    val activeIcon: ImageVector
)

@Composable
fun GlassBottomBar(
    tabs: List<TabItem>,
    selectedRoute: String,
    onTabSelected: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .navigationBarsPadding()
            .padding(horizontal = 12.dp, vertical = 8.dp),
        color = Color.Transparent,
        tonalElevation = 0.dp
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(76.dp)
                .clip(RoundedCornerShape(24.dp))
                .background(
                    Brush.verticalGradient(
                        colors = listOf(
                            Color(0xFF1A1A2E).copy(alpha = 0.92f),
                            Color(0xFF0F0F1A).copy(alpha = 0.96f)
                        )
                    )
                )
                .border(
                    BorderStroke(1.dp, Color.White.copy(alpha = 0.14f)),
                    RoundedCornerShape(24.dp)
                )
        ) {
            Row(
                modifier = Modifier.fillMaxWidth()
            ) {
                tabs.forEach { tab ->
                    GlassTabItem(
                        tab = tab,
                        isSelected = selectedRoute == tab.route,
                        onClick = { onTabSelected(tab.route) },
                        modifier = Modifier.weight(1f)
                    )
                }
            }
        }
    }
}

@Composable
private fun GlassTabItem(
    tab: TabItem,
    isSelected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()

    val targetScale = when {
        isPressed -> 0.96f
        isSelected -> 1.02f
        else -> 1f
    }

    val animatedScale by animateFloatAsState(
        targetValue = targetScale,
        animationSpec = spring(),
        label = "tabScale"
    )

    val iconAlpha = if (isSelected) 1f else 0.6f
    val textAlpha = if (isSelected) 1f else 0.7f

    Box(
        modifier = modifier
            .fillMaxHeight()
            .clickable(
                interactionSource = interactionSource,
                indication = null,
                onClick = onClick
            ),
        contentAlignment = Alignment.Center
    ) {
        val iconBackgroundModifier = Modifier
            .size(40.dp)
            .clip(RoundedCornerShape(14.dp))
            .then(
                if (isSelected) {
                    Modifier.background(
                        Brush.radialGradient(
                            colors = listOf(
                                Color(0xFF6C5CE7).copy(alpha = 0.25f),
                                Color.Transparent
                            )
                        )
                    )
                } else {
                    Modifier.background(Color.Transparent)
                }
            )

        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
            modifier = Modifier.scale(animatedScale)
        ) {
            Box(
                modifier = iconBackgroundModifier,
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = if (isSelected) tab.activeIcon else tab.icon,
                    contentDescription = tab.title,
                    tint = Color.White.copy(alpha = iconAlpha),
                    modifier = Modifier.size(24.dp)
                )
            }

            Text(
                text = tab.title,
                fontSize = 11.sp,
                fontWeight = if (isSelected) FontWeight.Bold else FontWeight.Medium,
                color = Color.White.copy(alpha = textAlpha),
                modifier = Modifier.padding(top = 4.dp)
            )
        }
    }
}
