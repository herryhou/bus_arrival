package com.busarrival.app.data.local.entity

import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey
import com.busarrival.app.data.pipeline.types.TimestampMs

@Entity(
    tableName = "departures",
    indices = [
        Index(value = ["timestamp"]),
        Index(value = ["stopIndex"]),
        Index(value = ["routeId"])
    ]
)
data class DepartureEntity(
    @PrimaryKey(autoGenerate = true)
    val id: Long = 0,
    val timestamp: TimestampMs,
    val stopIndex: Int,
    val sCm: Int,
    val dwellTimeS: Int,
    val routeId: String
)
