package com.busarrival.app.presentation.ui.history.components

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.busarrival.app.presentation.viewmodel.TimeFilter

@Composable
fun FilterChips(
    currentFilter: TimeFilter,
    onFilterSelected: (TimeFilter) -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        TimeFilter.values().forEach { filter ->
            FilterChip(
                selected = currentFilter == filter,
                onClick = { onFilterSelected(filter) },
                label = {
                    Text(
                        when (filter) {
                            TimeFilter.Today -> "Today"
                            TimeFilter.Week -> "Week"
                            TimeFilter.All -> "All"
                        }
                    )
                },
                modifier = Modifier.padding(end = 8.dp)
            )
        }
    }
}
