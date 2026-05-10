package com.busarrival.app.data.local.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.busarrival.app.data.local.entity.ArrivalEntity
import kotlinx.coroutines.flow.Flow

@Dao
interface ArrivalDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(arrival: ArrivalEntity): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(arrivals: List<ArrivalEntity>)

    @Query("SELECT * FROM arrivals WHERE timestamp >= :startTime AND timestamp <= :endTime ORDER BY timestamp ASC")
    fun getByTimeRange(startTime: Long, endTime: Long): Flow<List<ArrivalEntity>>

    @Query("SELECT * FROM arrivals WHERE routeId = :routeId ORDER BY timestamp DESC LIMIT :limit")
    fun getRecentByRoute(routeId: String, limit: Int): Flow<List<ArrivalEntity>>

    @Query("SELECT * FROM arrivals ORDER BY timestamp DESC LIMIT :limit")
    fun getRecent(limit: Int): Flow<List<ArrivalEntity>>

    @Query("SELECT stopIndex, COUNT(*) as count FROM arrivals GROUP BY stopIndex ORDER BY stopIndex")
    suspend fun countByStop(): List<StopCount>

    @Query("DELETE FROM arrivals WHERE timestamp < :beforeTime")
    suspend fun deleteOlderThan(beforeTime: Long): Int

    @Query("SELECT COUNT(*) FROM arrivals")
    suspend fun count(): Int
}

data class StopCount(
    val stopIndex: Int,
    val count: Int
)
