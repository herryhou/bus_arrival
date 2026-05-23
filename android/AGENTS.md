# Android App

Bus arrival display app for Android tablets.

## Build

```bash
cd android
./gradlew assembleDebug
./gradlew installDebug
```

## Key Files

- `app/src/main/` - Android app source
- `app/src/main/java/` - Kotlin code
- `docs/` - Android-specific docs (tile rendering, production build)

## Related Docs

- `GOOGLE_MAPS_SETUP.md` - Maps API setup
- `PRODUCTION_BUILD.md` - Release build instructions
- `REGRESSION_TEST_GUARANTEE.md` - Test commitments

## Root Docs

See project root `CLAUDE.md` for:
- Overall architecture
- Shared data formats (trace_v2.jsonl)
- Firmware/pipeline interface
