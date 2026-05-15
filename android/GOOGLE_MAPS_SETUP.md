# Google Maps API Key Setup

This app uses Google Maps for route visualization. You need to provide a Maps API key.

## Steps

1. **Get a Google Maps API Key**
   - Go to [Google Cloud Console](https://console.cloud.google.com/)
   - Create a project or select existing
   - Enable "Maps SDK for Android"
   - Create credentials → API key
   - Restrict the key:
     - Application: Android apps
     - Package name: `com.busarrival.app`
     - SHA-1 fingerprint: Use `keytool -list -v -keystore ~/.android/debug.keystore` for debug

2. **Add to local.properties**
   ```properties
   # In android/local.properties
   MAPS_API_KEY=your_api_key_here
   ```

3. **Build the app**
   ```bash
   ./gradlew assembleDebug
   ```

## Notes

- Never commit `local.properties` with API keys
- For production, use separate API keys with proper restrictions
- Consider using secrets management for CI/CD

## Troubleshooting

**Map shows blank grid**: API key missing or invalid
**"Unauthorized" error**: API key restrictions don't match app signature
**Map renders but no route**: Check route data loading in DetectionViewModel
