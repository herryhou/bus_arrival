package com.busarrival.app.data.local.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import com.busarrival.app.data.local.dao.ArrivalDao
import com.busarrival.app.data.local.dao.DepartureDao
import com.busarrival.app.data.local.entity.ArrivalEntity
import com.busarrival.app.data.local.entity.DepartureEntity

@Database(
    entities = [ArrivalEntity::class, DepartureEntity::class],
    version = 1,
    exportSchema = true
)
abstract class AppDatabase : RoomDatabase() {
    abstract fun arrivalDao(): ArrivalDao
    abstract fun departureDao(): DepartureDao

    companion object {
        private const val DATABASE_NAME = "bus_arrival.db"

        @Volatile
        private var INSTANCE: AppDatabase? = null

        fun getInstance(context: Context): AppDatabase {
            return INSTANCE ?: synchronized(this) {
                INSTANCE ?: buildDatabase(context).also { INSTANCE = it }
            }
        }

        private fun buildDatabase(context: Context): AppDatabase {
            return Room.databaseBuilder(
                context.applicationContext,
                AppDatabase::class.java,
                DATABASE_NAME
            )
                .fallbackToDestructiveMigration()
                .build()
        }
    }
}
