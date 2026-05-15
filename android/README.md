# Bus Arrival Android App

Pure Kotlin Android app for offline bus arrival detection.

## Build

```bash
./gradlew build
```

## Run

```bash
./gradlew installDebug
```

Or open in Android Studio and run.

## Architecture

```
app/
├── presentation/       # UI layer (Compose + ViewModels)
├── domain/             # Business logic (models, use cases)
├── data/               # Data layer (repositories, database, pipeline)
└── service/            # Foreground service for GPS processing
```

## Progress

- [x] Project structure
- [ ] Semantic types
- [ ] Binary format parser
- [ ] Map matching
- [ ] Kalman filter
- [ ] Dead reckoning
- [ ] Probability model
- [ ] State machine
- [ ] Recovery
- [ ] Location service
- [ ] Room database
- [ ] UI screens
- [ ] Testing
