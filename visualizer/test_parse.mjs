import { readFileSync } from 'fs';

const ROUTE_NODE_SIZE = 24;
const HEADER_SIZE_V8 = 28;
const MAGIC = 0x42555341;

function parseRouteNode(dataView, offset) {
    return {
        x_cm: dataView.getInt32(offset, true),
        y_cm: dataView.getInt32(offset + 4, true),
        cum_dist_cm: dataView.getInt32(offset + 8, true),
        seg_len_mm: dataView.getInt32(offset + 12, true),
        dx_cm: dataView.getInt16(offset + 16, true),
        dy_cm: dataView.getInt16(offset + 18, true),
        heading_cdeg: dataView.getInt16(offset + 20, true),
        _pad: dataView.getInt16(offset + 22, true)
    };
}

function projectCmToLatLon(x_cm, y_cm, lat_avg_deg) {
  const EARTH_R_CM = 637100000;
  const FIXED_ORIGIN_LON_DEG = 120.0;
  const FIXED_ORIGIN_Y_CM = 222389853;

  const avg_lat_rad = (lat_avg_deg * Math.PI) / 180;
  const lon_rad = (FIXED_ORIGIN_LON_DEG * Math.PI) / 180 + x_cm / (EARTH_R_CM * Math.cos(avg_lat_rad));
  const lat_rad = (y_cm + FIXED_ORIGIN_Y_CM) / EARTH_R_CM;

  return [(lat_rad * 180) / Math.PI, (lon_rad * 180) / Math.PI];
}

try {
    const buffer = readFileSync('../test_data/ty225_normal.bin');
    const dataView = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);

    // Check header
    const magic = dataView.getUint32(0, true);
    const version = dataView.getUint16(4, true);
    const node_count = dataView.getUint16(6, true);
    const stop_count = dataView.getUint8(8);

    console.log('Binary file header:');
    console.log('  Magic:', '0x' + magic.toString(16));
    console.log('  Version:', version);
    console.log('  Node count:', node_count);
    console.log('  Stop count:', stop_count);

    // Read lat_avg_deg
    const lat_avg_deg = dataView.getFloat64(20, true);

    // Parse all nodes
    const nodes = [];
    let offset = HEADER_SIZE_V8;
    for (let i = 0; i < node_count; i++) {
        nodes.push(parseRouteNode(dataView, offset));
        offset += ROUTE_NODE_SIZE;
    }

    console.log('\nParsed nodes:', nodes.length);

    // Generate route geometry
    const routeGeo = nodes.map((node) => {
        const [lat, lon] = projectCmToLatLon(node.x_cm, node.y_cm, lat_avg_deg);
        return [lon, lat];
    });

    console.log('Route geometry coordinates:', routeGeo.length);
    console.log('\nFirst 10 coordinates:');
    for (let i = 0; i < 10; i++) {
        const [lon, lat] = routeGeo[i];
        console.log('  Coord', i, ': lon=' + lon.toFixed(5) + ', lat=' + lat.toFixed(5));
    }

    console.log('\nLast 5 coordinates:');
    for (let i = routeGeo.length - 5; i < routeGeo.length; i++) {
        const [lon, lat] = routeGeo[i];
        console.log('  Coord', i, ': lon=' + lon.toFixed(5) + ', lat=' + lat.toFixed(5));
    }

} catch (error) {
    console.error('Error:', error.message);
}
