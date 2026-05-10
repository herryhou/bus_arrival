# Production Build Guide

## Signing Configuration

### Creating a Release Keystore

```bash
keytool -genkey -v -keystore release.keystore -keyalg RSA -keysize 2048 -validity 10000 -alias busarrival
```

### Setting Up keystore.properties

1. Copy `keystore.properties.example` to `keystore.properties`
2. Fill in the actual values from your keystore

**IMPORTANT:** Never commit `keystore.properties` or any keystore files!

## Building Release APK

```bash
# Build release APK
./gradlew assembleRelease

# Output: app/build/outputs/apk/release/app-release.apk
```

## Building Release Bundle (AAB)

```bash
# Build release bundle for Play Store
./gradlew bundleRelease

# Output: app/build/outputs/bundle/release/app-release.aab
```

## Build Variants

| Variant | Application ID | Description |
|---------|----------------|-------------|
| debug | com.busarrival.app.debug | Debug builds with logging |
| release | com.busarrival.app | Signed, minified release |

## Version Management

Update version in `app/build.gradle.kts`:

```kotlin
defaultConfig {
    versionCode = 1        // Increment for each release
    versionName = "1.0.0"  // Semantic versioning
}
```

## ProGuard

Release builds are automatically minified and obfuscated using rules in `proguard-rules.pro`.

## Testing Before Release

```bash
# Run unit tests
./gradlew test

# Run instrumentation tests
./gradlew connectedAndroidTest

# Run lint
./gradlew lint
```

## Play Store Configuration

### Required Permissions

The app uses:
- `ACCESS_FINE_LOCATION` - For GPS tracking
- `ACCESS_COARSE_LOCATION` - For network location fallback
- `ACCESS_BACKGROUND_LOCATION` - For foreground service
- `FOREGROUND_SERVICE` - For detection service
- `FOREGROUND_SERVICE_LOCATION` - Service type
- `POST_NOTIFICATIONS` - For service notifications
- `WAKE_LOCK` - For service lifecycle

### Privacy Policy

Ensure you have a privacy policy that explains:
- Location data collection
- Data storage (local only)
- Background location usage
- User data handling

## Release Checklist

- [ ] Update versionCode and versionName
- [ ] Run all tests (unit + UI)
- [ ] Test on physical device
- [ ] Verify Google Maps API key restrictions
- [ ] Check ProGuard rules
- [ ] Verify signing configuration
- [ ] Generate signed APK/AAB
- [ ] Test installed APK
- [ ] Prepare store listing (screenshots, description)
- [ ] Review privacy policy
