package com.busarrival.app.data.repository

import android.content.Context
import com.busarrival.app.data.local.dao.StopCount
import com.busarrival.app.data.local.db.AppDatabase
import com.busarrival.app.data.local.entity.ArrivalEntity
import com.busarrival.app.data.local.entity.DepartureEntity
import com.busarrival.app.domain.model.ArrivalEvent
import com.busarrival.app.domain.model.DepartureEvent
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

class DetectionRepository(context: Context) {

    private val database = AppDatabase.getInstance(context)
    private val arrivalDao = database.arrivalDao()
    private val departureDao = database.departureDao()

    suspend fun insertArrival(event: ArrivalEvent, routeId: String = "default") {
        val entity = ArrivalEntity(
            timestamp = event.timestamp,
            stopIndex = event.stopIndex,
            sCm = event.sCm,
            probability = event.probability.value,
            routeId = routeId
        )
        arrivalDao.insert(entity)
    }

    suspend fun insertDeparture(event: DepartureEvent, routeId: String = "default") {
        val entity = DepartureEntity(
            timestamp = event.timestamp,
            stopIndex = event.stopIndex,
            sCm = event.sCm,
            dwellTimeS = event.dwellTimeS,
            routeId = routeId
        )
        departureDao.insert(entity)
    }

    fun getArrivalsByTimeRange(startTime: Long, endTime: Long): Flow<List<ArrivalEntity>> {
        return arrivalDao.getByTimeRange(startTime, endTime)
    }

    fun getDeparturesByTimeRange(startTime: Long, endTime: Long): Flow<List<DepartureEntity>> {
        return departureDao.getByTimeRange(startTime, endTime)
    }

    fun getRecentArrivals(limit: Int = 50): Flow<List<ArrivalEntity>> {
        return arrivalDao.getRecent(limit)
    }

    fun getRecentDepartures(limit: Int = 50): Flow<List<DepartureEntity>> {
        return departureDao.getRecent(limit)
    }

    suspend fun getArrivalCountByStop(): List<StopCount> {
        return arrivalDao.countByStop()
    }

    suspend fun getDepartureCountByStop(): List<StopCount> {
        return departureDao.countByStop()
    }

    suspend fun cleanupOldRecords(retentionMs: Long): CleanupResult {
        val cutoffTime = System.currentTimeMillis() - retentionMs
        val arrivalsDeleted = arrivalDao.deleteOlderThan(cutoffTime)
        val departuresDeleted = departureDao.deleteOlderThan(cutoffTime)
        return CleanupResult(arrivalsDeleted, departuresDeleted)
    }

    suspend fun getTotalCounts(): TotalCounts {
        val arrivalCount = arrivalDao.count()
        val departureCount = departureDao.count()
        return TotalCounts(arrivalCount, departureCount)
    }
}

data class CleanupResult(
    val arrivalsDeleted: Int,
    val departuresDeleted: Int
)

data class TotalCounts(
    val arrivalCount: Int,
    val departureCount: Int
)
