# NMEA Format Reference

## Sentence Structure

NMEA sentences start with `$` and end with `*hh` (hex checksum):

```
$GPRMC,HHMMSS,A,DDMM.MMMM,N,DDDMM.MMMM,E,SSS.S,HHH.H,DDMYY,,*hh
$GPGGA,HHMMSS,DDMM.MMMM,N,DDDMM.MMMM,E,1,08,1.5,10.0,M,0.0,M,,*hh
```

## GPRMC (Recommended Minimum)

**Fields:**
- `$GPRMC` - Sentence identifier
- `HHMMSS` - UTC time
- `A` - Status (A=valid, V=invalid)
- `DDMM.MMMM,N` - Latitude (degrees + minutes, N/S)
- `DDDMM.MMMM,E` - Longitude (degrees + minutes, E/W)
- `SSS.S` - Speed in knots
- `HHH.H` - Heading in degrees (0-360°) **← Critical field**
- `DDMYY` - Date
- `*hh` - Checksum

## GPGGA (Fix Data)

**Fields:**
- `$GPGGA` - Sentence identifier
- `HHMMSS` - UTC time
- `DDMM.MMMM,N` - Latitude
- `DDDMM.MMMM,E` - Longitude
- `1` - Fix quality (1=GPS)
- `08` - Satellites
- `1.5` - HDOP
- `10.0,M` - Altitude
- `*hh` - Checksum

## Checksum Calculation

XOR all bytes between `$` and `*`:

```javascript
function nmeaChecksum(sentence) {
  let cs = 0;
  for (let i = 0; i < sentence.length; i++) 
    cs ^= sentence.charCodeAt(i);
  return cs.toString(16).toUpperCase().padStart(2, '0');
}
```

## Heading Field

**Range:** 0-360° (true course)
- 0° = North
- 90° = East
- 180° = South
- 270° = West
- 350.5° = Northwest (overflows if ×100)

**Conversion to centidegrees:**
```rust
// NMEA heading (0-360) to i16 (-18000 to 18000)
let heading_cdeg = (heading_deg * 100.0).round() as i32;
if heading_cdeg > 18000 {
    heading_cdeg -= 36000;  // Wrap to negative range
}
```
