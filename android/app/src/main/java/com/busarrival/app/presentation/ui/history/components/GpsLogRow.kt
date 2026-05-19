package com.busarrival.app.presentation.ui.history.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.busarrival.app.presentation.viewmodel.LogManagerItem
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun GpsLogRow(
    item: LogManagerItem,
    onToggle: (String) -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onToggle(item.reference) }
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Checkbox(
                checked = item.isSelected,
                onCheckedChange = { onToggle(item.reference) }
            )

            Column(modifier = Modifier.weight(1f)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text(
                        text = item.filename,
                        style = MaterialTheme.typography.titleMedium
                    )
                    if (item.isActive) {
                        AssistChip(
                            onClick = { },
                            label = { Text("Active") }
                        )
                    }
                }

                Text(
                    text = formatTimestamp(item.modifiedAtMillis),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                Text(
                    text = formatSize(item.sizeBytes),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

private fun formatTimestamp(millis: Long): String {
    val formatter =
        SimpleDateFormat("yyyy-MM-dd HH:mm", Locale.getDefault()).apply {
            timeZone = java.util.TimeZone.getDefault()
        }
    return formatter.format(Date(millis))
}

private fun formatSize(sizeBytes: Long): String {
    if (sizeBytes < 1024) return "$sizeBytes B"
    val kib = sizeBytes / 1024.0
    if (kib < 1024) return String.format(Locale.getDefault(), "%.1f KiB", kib)
    val mib = kib / 1024.0
    return String.format(Locale.getDefault(), "%.1f MiB", mib)
}
