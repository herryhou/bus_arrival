# Android Mock Location Guide for NMEA Simulation

## Overview

Use Android's built-in mock location system to simulate GPS from NMEA files in the bus arrival detection app.

## Prerequisites

- Android device/emulator with API 23+
- Developer mode enabled
- ADB installed

## Setup

### 1. Enable Mock Location Permission

```bash
# Grant mock location permission to your app
adb shell appops set com.busarrival.app android:mock_location allow

# Verify permission
adb shell appops get com.busarrival.app android:mock_location
# Should show: android:mock_location: allow
```

### 2. Add Permission to Manifest (if not present)

```xml
<!-- AndroidManifest.xml -->
<uses-permission android:name="android.permission.ACCESS_MOCK_LOCATION"
    tools:ignore="MockLocation,ProtectedPermissions" />
```

### 3. Create NMEA Player Service

Create `MockGpsService.kt`:

```kotlin
package com.busarrival.app.service

import android.annotation.SuppressLint
import android.app.Service
import android.content.Context
import android.content.Intent
import android.location.Location
import android.location.LocationManager
import android.os.IBinder
import android.os.SystemClock
import kotlinx.coroutines.*
import java.io.File

class MockGpsService : Service() {
    private val scope = CoroutineScope(Dispatchers.Default + Job())
    private var playbackJob: Job? = null
    private lateinit var locationManager: LocationManager

    override fun onCreate() {
        super.onCreate()
        locationManager = getSystemService(Context.LOCATION_SERVICE) as LocationManager
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val nmeaPath = intent?.getStringExtra(EXTRA_NMEA_PATH)
        val speedMultiplier = intent?.getFloatExtra(EXTRA_SPEED, 1.0f) ?: 1.0f

        if (nmeaPath != null) {
            startPlayback(nmeaPath, speedMultiplier)
        }

        return START_STICKY
    }

    @SuppressLint("MissingPermission")
    private fun startPlayback(nmeaPath: String, speedMultiplier: Float) {
        playbackJob = scope.launch {
            val locations = NmeaParser.parseFile(File(nmeaPath).readText())

            var lastTime = locations.firstOrNull()?.time ?: 0L

            for (location in locations) {
                // Calculate delay based on timestamp difference
                val timeDiff = if (lastTime > 0) location.time - lastTime else 1000L
                val delay = (timeDiff / speedMultiplier).toLong()

                delay(delay.coerceAtLeast(10)) // Min 10ms delay

                // Inject mock location
                injectMockLocation(location)

                lastTime = location.time
            }

            // Playback complete
            stopSelf()
        }
    }

    @SuppressLint("MissingPermission")
    private fun injectMockLocation(location: Location) {
        val mockLocation = Location(location).apply {
            // Make it a mock location
            putExtra(Location.EXTRA_IS_MOCK_LOCATION, true)
            // Use elapsed realtime for current timestamp
            time = SystemClock.elapsedRealtimeNanos() / 1_000_000
        }

        locationManager.setTestProviderEnabled(LocationManager.GPS_PROVIDER, true)
        locationManager.setTestProviderStatus(LocationManager.GPS_PROVIDER, 2, null, SystemClock.elapsedRealtimeNanos())
        locationManager.setTestProviderLocation(LocationManager.GPS_PROVIDER, mockLocation)
    }

    override fun onDestroy() {
        super.onDestroy()
        playbackJob?.cancel()

        // Clean up test provider
        try {
            locationManager.removeTestProvider(LocationManager.GPS_PROVIDER)
        } catch (e: Exception) {
            // Provider already removed
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val EXTRA_NMEA_PATH = "nmea_path"
        const val EXTRA_SPEED = "speed_multiplier"

        fun startPlayback(context: Context, nmeaPath: String, speedMultiplier: Float = 1.0f) {
            val intent = Intent(context, MockGpsService::class.java).apply {
                putExtra(EXTRA_NMEA_PATH, nmeaPath)
                putExtra(EXTRA_SPEED, speedMultiplier)
            }
            context.startForegroundService(intent)
        }

        fun stopPlayback(context: Context) {
            context.stopService(Intent(context, MockGpsService::class.java))
        }
    }
}
```

### 4. Add Service to Manifest

```xml
<!-- AndroidManifest.xml -->
<service android:name=".service.MockGpsService" />
```

### 5. Create UI Controls

Add to `DetectionScreen.kt`:

```kotlin
// In DetectionScreen composable
Row(
    modifier = Modifier.padding(16.dp),
    horizontalArrangement = Arrangement.spacedBy(8.dp)
) {
    Button(onClick = {
        // Start NMEA playback
        val nmeaPath = copyNmeaToCache("ty225_normal_nmea.txt")
        MockGpsService.startPlayback(context, nmeaPath, 1.0f)
    }) {
        Text("Play NMEA")
    }

    Button(onClick = {
        // Stop playback
        MockGpsService.stopPlayback(context)
    }) {
        Text("Stop")
    }

    Button(onClick = {
        // Speed up playback
        val nmeaPath = copyNmeaToCache("ty225_normal_nmea.txt")
        MockGpsService.startPlayback(context, nmeaPath, 10.0f) // 10x speed
    }) {
        Text("Fast Forward")
    }
}

// Helper function to copy NMEA from assets to cache
fun copyNmeaToCache(filename: String): String {
    val cacheFile = File(context.cacheDir, filename)
    if (!cacheFile.exists()) {
        context.assets.open(filename).use { input ->
            cacheFile.outputStream().use { output ->
                input.copyTo(output)
            }
        }
    }
    return cacheFile.absolutePath
}
```

## Usage

### Option 1: Push NMEA File to Device

```bash
# Push NMEA file to app's cache
adb push test_data/ty225_normal_nmea.txt /sdcard/

# Grant permissions and start playback
adb shell appops set com.busarrival.app android:mock_location allow
adb shell am startservice -a com.busarrival.app.START_MOCK_GPS \
    --es nmea_path /sdcard/ty225_normal_nmea.txt \
    --ef speed 1.0
```

### Option 2: Use App UI

1. Copy NMEA files to `android/app/src/main/assets/`
2. Build and install app
3. Open app → Detection Screen
4. Tap "Play NMEA" button
5. Watch real-time detection on map

### Option 3: ADB Commands (for emulator)

```bash
# Connect to emulator console
adb shell

# Set mock location manually (single point)
am broadcast -a android.location.FUSED_LOCATION \
    --es latitude "25.0" \
    --es longitude "121.0" \
    --es speed "5.0" \
    --es bearing "90.0"
```

## Testing Checklist

- [ ] Mock location permission granted
- [ ] NMEA file accessible to app
- [ ] MockGpsService registered in manifest
- [ ] DetectionService receives mock locations
- [ ] Map updates in real-time
- [ ] Arrivals/departures detected correctly
- [ ] Trace file records mock locations

## Troubleshooting

### "Mock location not allowed"

```bash
adb shell appops set com.busarrival.app android:mock_location allow
```

### "Test provider doesn't exist"

Add to `onCreate()` before first injection:

```kotlin
locationManager.addTestProvider(
    LocationManager.GPS_PROVIDER,
    false, false, false, false,
    true, true, true,
    android.location.Criteria.POWER_LOW,
    android.location.Criteria.ACCURACY_FINE
)
```

### Locations not appearing

Check `DetectionService` is listening for location updates:

```kotlin
// Should work with both real and mock locations
locationManager.requestLocationUpdates(
    LocationManager.GPS_PROVIDER,
    1000L,  // 1 second interval
    1.0f,   // 1 meter min distance
    locationCallback,
    Looper.getMainLooper()
)
```

### Playback too slow/fast

Adjust speed multiplier:

```kotlin
MockGpsService.startPlayback(context, nmeaPath, speedMultiplier = 10.0f) // 10x
```

## Alternative: External Mock Location Apps

If you don't want to implement the service:

1. **GPS Joystick** - Route planning from file
2. **Fake GPS Go** - Simple location spoofing
3. **Lock GPS** - Mock location with custom routes

Usage:
1. Install mock location app
2. Set as "Mock Location App" in Developer Options
3. Import NMEA file (convert to GPX if needed)
4. Start playback
5. Open bus arrival app

## NMEA to GPX Conversion

If external app requires GPX format:

```python
# nmea_to_gpx.py
import sys
from datetime import datetime

def convert_nmea_to_gpx(nmea_file, gpx_file):
    print(f'<?xml version="1.0" encoding="UTF-8"?>')
    print(f'<gpx version="1.1" creator="BusArrival">')
    print(f'  <trk>')
    print(f'    <name>Bus Route</name>')
    print(f'    <trkseg>')

    with open(nmea_file) as f:
        for line in f:
            if line.startswith('$GPRMC'):
                parts = line.split(',')
                if len(parts) >= 10 and parts[2] == 'A':
                    lat = parse_lat_lon(parts[3], parts[4])
                    lon = parse_lat_lon(parts[5], parts[6])
                    time = parts[1]

                    print(f'      <trkpt lat="{lat}" lon="{lon}">')
                    print(f'        <time>{format_time(time)}</time>')
                    print(f'      </trkpt>')

    print(f'    </trkseg>')
    print(f'  </trk>')
    print(f'</gpx>')

if __name__ == '__main__':
    convert_nmea_to_gpx(sys.argv[1], sys.argv[2])
```

Run:
```bash
python3 nmea_to_gpx.py ty225_normal_nmea.txt route.gpx
```
