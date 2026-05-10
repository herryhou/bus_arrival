package com.busarrival.app.data.local.dao

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.busarrival.app.data.local.db.AppDatabase
import com.busarrival.app.data.local.entity.ArrivalEntity
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue

@RunWith(AndroidJUnit4::class)
class ArrivalDaoTest {

    private lateinit var database: AppDatabase
    private lateinit var arrivalDao: ArrivalDao

    @Before
    fun setup() {
        database = Room.inMemoryDatabaseBuilder(
            ApplicationProvider.getApplicationContext(),
            AppDatabase::class.java
        ).build()
        arrivalDao = database.arrivalDao()
    }

    @After
    fun teardown() {
        database.close()
    }

    @Test
    fun insertAndRetrieveArrival() = runTest {
        val arrival = ArrivalEntity(
            timestamp = 1625097600000,
            stopIndex = 5,
            sCm = 12345,
            probability = 200,
            routeId = "ty225"
        )

        arrivalDao.insert(arrival)

        val retrieved = arrivalDao.getRecent(1).first()
        assertEquals(1, retrieved.size)
        assertEquals(5, retrieved[0].stopIndex)
        assertEquals(200, retrieved[0].probability)
    }

    @Test
    fun getArrivalsByTimeRange() = runTest {
        val now = System.currentTimeMillis()
        val hourMs = 3600000L

        val arrivals = listOf(
            ArrivalEntity(0, now - hourMs, 1, 1000, 150, "route1"),
            ArrivalEntity(0, now, 2, 2000, 180, "route1"),
            ArrivalEntity(0, now + hourMs, 3, 3000, 200, "route1")
        )

        arrivalDao.insertAll(arrivals)

        val result = arrivalDao.getByTimeRange(now - hourMs, now).first()
        assertEquals(2, result.size)
        assertEquals(1, result[0].stopIndex)
        assertEquals(2, result[1].stopIndex)
    }

    @Test
    fun countByStop() = runTest {
        val arrivals = listOf(
            ArrivalEntity(0, 1000, 1, 1000, 150, "route1"),
            ArrivalEntity(0, 2000, 1, 1000, 150, "route1"),
            ArrivalEntity(0, 3000, 2, 1000, 150, "route1")
        )

        arrivalDao.insertAll(arrivals)

        val counts = arrivalDao.countByStop()
        assertEquals(2, counts.size)
        assertEquals(2, counts.find { it.stopIndex == 1 }?.count)
        assertEquals(1, counts.find { it.stopIndex == 2 }?.count)
    }

    @Test
    fun deleteOlderThan() = runTest {
        val now = System.currentTimeMillis()
        val dayMs = 86400000L

        val arrivals = listOf(
            ArrivalEntity(0, now - 2 * dayMs, 1, 1000, 150, "route1"),
            ArrivalEntity(0, now - dayMs, 2, 1000, 150, "route1"),
            ArrivalEntity(0, now, 3, 1000, 150, "route1")
        )

        arrivalDao.insertAll(arrivals)

        val deleted = arrivalDao.deleteOlderThan(now - dayMs)
        assertEquals(1, deleted)

        val remaining = arrivalDao.count()
        assertEquals(2, remaining)
    }

    @Test
    fun getRecentByRoute() = runTest {
        val now = System.currentTimeMillis()

        val arrivals = listOf(
            ArrivalEntity(0, now, 1, 1000, 150, "route1"),
            ArrivalEntity(0, now + 1000, 2, 1000, 150, "route2"),
            ArrivalEntity(0, now + 2000, 3, 1000, 150, "route1")
        )

        arrivalDao.insertAll(arrivals)

        val route1Arrivals = arrivalDao.getRecentByRoute("route1", 10).first()
        assertEquals(2, route1Arrivals.size)
        assertTrue(route1Arrivals.all { it.routeId == "route1" })
    }
}
