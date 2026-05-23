package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.busarrival.app.domain.model.EventHint
import kotlinx.coroutines.delay

@Composable
fun EventToastHost(
    hint: EventHint?,
    modifier: Modifier = Modifier
) {
    var visible by remember { mutableStateOf(false) }

    LaunchedEffect(hint) {
        if (hint != null) {
            visible = true
            delay(5000) // Auto-dismiss after 5 seconds
            visible = false
        } else {
            visible = false
        }
    }

    Box(
        modifier = modifier.padding(16.dp),
        contentAlignment = Alignment.TopCenter
    ) {
        AnimatedVisibility(
            visible = visible && hint != null,
            enter = slideInVertically(
                initialOffsetY = { -it },
                animationSpec = androidx.compose.animation.core.tween(300)
            ) + fadeIn(animationSpec = androidx.compose.animation.core.tween(300)),
            exit = slideOutVertically(
                targetOffsetY = { -it },
                animationSpec = androidx.compose.animation.core.tween(300)
            ) + fadeOut(animationSpec = androidx.compose.animation.core.tween(300))
        ) {
            hint?.let {
                EventToast(hint = it)
            }
        }
    }
}
