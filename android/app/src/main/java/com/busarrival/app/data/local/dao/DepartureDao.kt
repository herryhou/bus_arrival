package com.busarrival.app.data.local.dao

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.busarrival.app.data.local.entity.DepartureEntity
import kotlinx.coroutines.flow.Flow

@Dao
interface DepartureDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(departure: DepartureEntity): Long

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(departures: List<DepartureEntity>)

    @Query("SELECT * FROM departures WHERE timestamp >= :startTime AND timestamp <= :endTime ORDER BY timestamp ASC")
    fun getByTimeRange(startTime: Long, endTime: Long): Flow<List<DepartureEntity>>

    @Query("SELECT * FROM departures WHERE routeId = :routeId ORDER BY timestamp DESC LIMIT :limit")
    fun getRecentByRoute(routeId: String, limit: Int): Flow<List<DepartureEntity>>

    @Query("SELECT * FROM departures ORDER BY timestamp DESC LIMIT :limit")
    fun getRecent(limit: Int): Flow<List<DepartureEntity>>

    @Query("SELECT stopIndex, COUNT(*) as count FROM departures GROUP BY stopIndex ORDER BY stopIndex")
    suspend fun countByStop(): List<StopCount>

    @Query("DELETE FROM departures WHERE timestamp < :beforeTime")
    suspend fun deleteOlderThan(beforeTime: Long): Int

    @Query("SELECT COUNT(*) FROM departures")
    suspend fun count(): Int
}
