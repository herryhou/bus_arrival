#!/usr/bin/env python3
"""Generate NMEA test data with proper checksums for heading overflow stress test"""

import math

def nmea_checksum(sentence):
    """Calculate NMEA checksum"""
    data = sentence[1:]  # Skip the $
    checksum = 0
    for char in data:
        checksum ^= ord(char)
    return checksum

def generate_rmc(time_str, lat, lat_ns, lon, lon_ew, speed_knots, heading_true, date_str):
    """Generate $GPRMC sentence with proper checksum"""
    # Format: $GPRMC,HHMMSS,A,DDMM.MMMM,N,DDDMM.MMMM,E,SSS.S,HHH.H,DDMYY,,,*hh
    # Format numbers
    lat_deg = int(abs(lat))
    lat_min = (abs(lat) - lat_deg) * 60
    lat_str = f"{lat_deg:02d}{lat_min:07.4f}".replace('.', '')

    lon_deg = int(abs(lon))
    lon_min = (abs(lon) - lon_deg) * 60
    lon_str = f"{lon_deg:03d}{lon_min:07.4f}".replace('.', '')

    sentence = f"$GPRMC,{time_str},A,{lat_str},{lat_ns},{lon_str},{lon_ew},{speed_knots:.1f},{heading_true:.1f},{date_str},,"
    checksum = nmea_checksum(sentence)
    return f"{sentence}*{checksum:02X}"

def generate_gga(time_str, lat, lat_ns, lon, lon_ew):
    """Generate $GPGGA sentence with proper checksum"""
    lat_deg = int(abs(lat))
    lat_min = (abs(lat) - lat_deg) * 60
    lat_str = f"{lat_deg:02d}{lat_min:07.4f}".replace('.', '')

    lon_deg = int(abs(lon))
    lon_min = (abs(lon) - lon_deg) * 60
    lon_str = f"{lon_deg:03d}{lon_min:07.4f}".replace('.', '')

    sentence = f"$GPGGA,{time_str},{lat_str},{lat_ns},{lon_str},{lon_ew},1,08,1.5,10.0,M,0.0,M,,"
    checksum = nmea_checksum(sentence)
    return f"{sentence}*{checksum:02X}"

# Generate stress test NMEA data
# Base coordinates around 25.0000, 121.0000
date_str = "010100"  # Jan 1, 2001

# Test headings: we want to cross 180° boundary rapidly
# Also include 350° to test original bug
test_scenarios = [
    (0, 0.0, 0.0),      # Start: 0° heading
    (2, 0.0001, 0.0005),  # 80° heading
    (4, 0.0002, 0.0010),  # 179° heading (just before overflow)
    (6, 0.0002, 0.0010),  # 180° heading (boundary)
    (8, 0.0002, 0.0010),  # 181° heading (crosses to negative)
    (10, 0.0002, 0.0010), # 182° heading (well into negative)
    (12, 0.0002, 0.0010), # 179° heading (back to positive)
    (14, 0.0002, 0.0005), # 270° heading
    (16, 0.0003, 120.9995), # 270° heading again
    (18, 0.0004, 121.0000), # 0° heading (north)
    (20, 0.0005, 121.0005), # 45° heading
    (22, 0.0006, 121.0010), # 90° heading
    (24, 0.0006, 121.0010), # 135° heading
    (26, 0.0006, 121.0010), # 179° heading (boundary again)
    (28, 0.0006, 121.0010), # 181° heading (cross)
    (30, 0.0007, 121.0005), # 270° heading
    (32, 0.0008, 121.0000), # 350° heading (original bug case!)
    (34, 0.0008, 121.0000), # 355° heading
    (36, 0.0008, 121.0000), # 359° heading (extreme case)
    (38, 0.0008, 121.0000), # 359.9° heading (almost 360)
    (40, 0.0009, 121.0005), # 45° heading back
]

headings = [
    0.0,      # North
    80.0,     # Northeast
    179.0,    # Almost south (positive)
    180.0,    # South (boundary)
    181.0,    # Past south (negative)
    182.0,    # Further into negative
    179.0,    # Back to positive
    270.0,    # West
    270.0,    # West again
    0.0,      # North
    45.0,     # Northeast
    90.0,     # East
    135.0,    # Southeast
    179.0,    # Near boundary
    181.0,    # Cross boundary
    270.0,    # West
    350.0,    # Original bug case (350.5° × 100 = 35050 > 32767)
    355.0,    # Northwest
    359.0,    # Almost north
    359.9,    # Extreme case
    45.0,     # Return to normal
]

output = []
for i, (time, lat, lon) in enumerate(test_scenarios):
    time_str = f"{time:06d}"
    heading = headings[i]

    rmc = generate_rmc(time_str, lat, 'N', lon, 'E', 5.0, heading, date_str)
    gga = generate_gga(time_str, lat, 'N', lon, 'E')

    output.append(rmc)
    output.append(gga)

# Write to file
with open('heading_stress_test.nmea', 'w') as f:
    for line in output:
        f.write(line + '\n')

print(f"Generated {len(output)} NMEA lines")
print("Test headings:", set(headings))
print("\nExpected heading_cdeg values:")
for h in set(headings):
    hc = int(h * 100)
    if hc > 18000:
        hc -= 36000
    print(f"  {h:6.1f}° → {hc:6} cdeg")
