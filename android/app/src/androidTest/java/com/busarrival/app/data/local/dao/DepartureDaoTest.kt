package com.busarrival.app.data.local.dao

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.busarrival.app.data.local.db.AppDatabase
import com.busarrival.app.data.local.entity.DepartureEntity
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue

@RunWith(AndroidJUnit4::class)
class DepartureDaoTest {

    private lateinit var database: AppDatabase
    private lateinit var departureDao: DepartureDao

    @Before
    fun setup() {
        database = Room.inMemoryDatabaseBuilder(
            ApplicationProvider.getApplicationContext(),
            AppDatabase::class.java
        ).build()
        departureDao = database.departureDao()
    }

    @After
    fun teardown() {
        database.close()
    }

    @Test
    fun insertAndRetrieveDeparture() = runTest {
        val departure = DepartureEntity(
            timestamp = 1625097600000,
            stopIndex = 5,
            sCm = 12345,
            dwellTimeS = 30,
            routeId = "ty225"
        )

        departureDao.insert(departure)

        val retrieved = departureDao.getRecent(1).first()
        assertEquals(1, retrieved.size)
        assertEquals(5, retrieved[0].stopIndex)
        assertEquals(30, retrieved[0].dwellTimeS)
    }

    @Test
    fun getDeparturesByTimeRange() = runTest {
        val now = System.currentTimeMillis()
        val hourMs = 3600000L

        val departures = listOf(
            DepartureEntity(0, now - hourMs, 1, 1000, 20, "route1"),
            DepartureEntity(0, now, 2, 2000, 30, "route1"),
            DepartureEntity(0, now + hourMs, 3, 3000, 25, "route1")
        )

        departureDao.insertAll(departures)

        val result = departureDao.getByTimeRange(now - hourMs, now).first()
        assertEquals(2, result.size)
        assertEquals(1, result[0].stopIndex)
        assertEquals(2, result[1].stopIndex)
    }

    @Test
    fun countByStop() = runTest {
        val departures = listOf(
            DepartureEntity(0, 1000, 1, 1000, 20, "route1"),
            DepartureEntity(0, 2000, 1, 1000, 25, "route1"),
            DepartureEntity(0, 3000, 2, 1000, 30, "route1")
        )

        departureDao.insertAll(departures)

        val counts = departureDao.countByStop()
        assertEquals(2, counts.size)
        assertEquals(2, counts.find { it.stopIndex == 1 }?.count)
        assertEquals(1, counts.find { it.stopIndex == 2 }?.count)
    }

    @Test
    fun deleteOlderThan() = runTest {
        val now = System.currentTimeMillis()
        val dayMs = 86400000L

        val departures = listOf(
            DepartureEntity(0, now - 2 * dayMs, 1, 1000, 20, "route1"),
            DepartureEntity(0, now - dayMs, 2, 1000, 25, "route1"),
            DepartureEntity(0, now, 3, 1000, 30, "route1")
        )

        departureDao.insertAll(departures)

        val deleted = departureDao.deleteOlderThan(now - dayMs)
        assertEquals(1, deleted)

        val remaining = departureDao.count()
        assertEquals(2, remaining)
    }

    @Test
    fun getRecentByRoute() = runTest {
        val now = System.currentTimeMillis()

        val departures = listOf(
            DepartureEntity(0, now, 1, 1000, 20, "route1"),
            DepartureEntity(0, now + 1000, 2, 1000, 25, "route2"),
            DepartureEntity(0, now + 2000, 3, 1000, 30, "route1")
        )

        departureDao.insertAll(departures)

        val route1Departures = departureDao.getRecentByRoute("route1", 10).first()
        assertEquals(2, route1Departures.size)
        assertTrue(route1Departures.all { it.routeId == "route1" })
    }
}
