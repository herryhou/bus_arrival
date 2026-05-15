package com.busarrival.app.data.local.entity

import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey

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
    val timestamp: Long,
    val stopIndex: Int,
    val sCm: Int,
    val dwellTimeS: Int,
    val routeId: String
)
