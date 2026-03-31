/**
 * Test script to verify TypeScript binary parser matches Rust format
 *
 * Run: node test_parser.mjs
 */

import { readFileSync } from 'fs';

const ROUTE_NODE_SIZE = 24;
const HEADER_SIZE = 28;

function parseRouteNode(dataView, offset) {
    return {
        x_cm: dataView.getInt32(offset, true),
        y_cm: dataView.getInt32(offset + 4, true),
        cum_dist_cm: dataView.getInt32(offset + 8, true),
        seg_len_mm: dataView.getInt32(offset + 12, true), // CRITICAL: i32, NOT i64!
        dx_cm: dataView.getInt16(offset + 16, true),
        dy_cm: dataView.getInt16(offset + 18, true),
        heading_cdeg: dataView.getInt16(offset + 20, true),
        _pad: dataView.getInt16(offset + 22, true)
    };
}

try {
    const buffer = readFileSync('/tmp/test_route.bin');
    const dataView = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);

    console.log('\n=== TypeScript Parser Verification ===\n');
    console.log('File size:', buffer.length, 'bytes');

    // Check header
    const magic = dataView.getUint32(0, true);
    const version = dataView.getUint16(4, true);
    const nodeCount = dataView.getUint16(6, true);
    const stopCount = dataView.getUint8(8);

    console.log('\nHeader:');
    console.log('  Magic:', '0x' + magic.toString(16), '(expected 0x42555341)');
    console.log('  Version:', version, '(expected 4)');
    console.log('  Node count:', nodeCount);
    console.log('  Stop count:', stopCount);

    // Parse first node
    const firstNode = parseRouteNode(dataView, HEADER_SIZE);
    console.log('\nFirst RouteNode:');
    console.log('  x_cm:', firstNode.x_cm);
    console.log('  y_cm:', firstNode.y_cm);
    console.log('  cum_dist_cm:', firstNode.cum_dist_cm);
    console.log('  seg_len_mm:', firstNode.seg_len_mm, '(i32)');
    console.log('  dx_cm:', firstNode.dx_cm);
    console.log('  dy_cm:', firstNode.dy_cm);
    console.log('  heading_cdeg:', firstNode.heading_cdeg);
    console.log('  _pad:', firstNode._pad);

    // These values should match the Rust test output:
    // x_cm: 12965481, y_cm: 55644721, cum_dist_cm: 0, seg_len_mm: 19863
    // dx_cm: -328, dy_cm: 1959, heading_cdeg: -951

    const expected = {
        x_cm: 12965481,
        y_cm: 55644721,
        cum_dist_cm: 0,
        seg_len_mm: 19863,
        dx_cm: -328,
        dy_cm: 1959,
        heading_cdeg: -951
    };

    let match = true;
    for (const key of Object.keys(expected)) {
        if (firstNode[key] !== expected[key]) {
            console.log(`\n❌ MISMATCH: ${key} = ${firstNode[key]}, expected ${expected[key]}`);
            match = false;
        }
    }

    if (match) {
        console.log('\n✓ All values match! TypeScript parser is correct.\n');
    } else {
        console.log('\n❌ Parser has errors. Check the field offsets and types.\n');
        process.exit(1);
    }

} catch (error) {
    console.error('Error:', error.message);
    process.exit(1);
}
