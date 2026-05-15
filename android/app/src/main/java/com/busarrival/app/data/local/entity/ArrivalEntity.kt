package com.busarrival.app.data.local.entity

import androidx.room.Entity
import androidx.room.Index
import androidx.room.PrimaryKey
import com.busarrival.app.data.pipeline.types.DistCm
import com.busarrival.app.data.pipeline.types.Prob8

@Entity(
    tableName = "arrivals",
    indices = [
        Index(value = ["timestamp"]),
        Index(value = ["stopIndex"]),
        Index(value = ["routeId"])
    ]
)
data class ArrivalEntity(
    @PrimaryKey(autoGenerate = true)
    val id: Long = 0,
    val timestamp: Long,
    val stopIndex: Int,
    val sCm: Int,
    val probability: Int,
    val routeId: String
)
