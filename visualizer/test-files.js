import { parseTraceJsonl } from './src/lib/parsers/trace.ts';
import { loadRouteData } from './src/lib/parsers/routeData.ts';
import { readFileSync } from 'fs';

async function test() {
  try {
    // Test route data
    const routeBuf = readFileSync('./static/tz_23_short.bin');
    const routeFile = new File([routeBuf], 'tz_23_short.bin');
    const routeData = await loadRouteData(routeFile);
    console.log('Route data:', { nodeCount: routeData.node_count, stopCount: routeData.stop_count });
    
    // Test trace data
    const traceContent = readFileSync('./static/tz_23_short_trace_v2.jsonl', 'utf8');
    const traceData = parseTraceJsonl(traceContent);
    console.log('Trace data:', { count: traceData.length, firstTime: traceData[0].gps.time_ms });
    
    console.log('SUCCESS: Both files loaded correctly');
  } catch (e) {
    console.error('ERROR:', e.message);
    process.exit(1);
  }
}

test();
