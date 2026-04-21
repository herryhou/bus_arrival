/// <reference lib="dom" />

/**
 * File System Access API wrapper for smart file discovery
 * Auto-discovers matching trace.jsonl when user selects a .bin file
 */

export interface FilePickerResult {
  routeFile: File;
  traceFile: File | null;
  autoDiscovered: boolean;
}

export interface FilePickerOptions {
  accept?: string;
  multiple?: boolean;
}

/**
 * Check if File System Access API is supported
 */
export function isFileSystemAccessAPISupported(): boolean {
  if (typeof window === 'undefined') return false;
  return 'showOpenFilePicker' in window;
}

/**
 * Extract basename from filename (without extension)
 */
export function extractBasename(filename: string): string {
  const lastDot = filename.lastIndexOf('.');
  return lastDot >= 0 ? filename.substring(0, lastDot) : filename;
}

/**
 * Generate possible trace filenames from route basename
 */
export function generateTraceFilenames(routeBasename: string): string[] {
  return [
    `${routeBasename}_trace.jsonl`,
    `${routeBasename}.jsonl`,
    `${routeBasename}_trace.json`,
    `${routeBasename}.json`,
  ];
}

/**
 * Pick a route file and auto-discover matching trace file
 */
export async function pickRouteFile(): Promise<FilePickerResult | null> {
  if (!isFileSystemAccessAPISupported()) {
    return null; // Fall back to traditional input
  }

  try {
    const [handle] = await window.showOpenFilePicker({
      types: [{
        description: 'Route Data',
        accept: { 'application/octet-stream': ['.bin'] }
      }],
      multiple: false
    });

    const routeFile = await handle.getFile();
    const routeBasename = extractBasename(routeFile.name);

    // Try to get parent directory handle
    let traceFile: File | null = null;
    let autoDiscovered = false;

    try {
      // Get directory handle (requires permission)
      const dirHandle = await handle.getParent?.();
      if (dirHandle) {
        for await (const entry of dirHandle.values()) {
          if (entry.kind === 'file') {
            const traceName = entry.name;
            const possibleNames = generateTraceFilenames(routeBasename);
            if (possibleNames.includes(traceName)) {
              const fileHandle = entry as FileSystemFileHandle;
              traceFile = await fileHandle.getFile();
              autoDiscovered = true;
              break;
            }
          }
        }
      }
    } catch (e) {
      // getParent() not supported or permission denied - no auto-discovery
      console.debug('Could not access parent directory:', e);
    }

    return { routeFile, traceFile, autoDiscovered };
  } catch (e) {
    // User cancelled or error
    if ((e as Error).name !== 'AbortError') {
      console.error('File picker error:', e);
    }
    return null;
  }
}

/**
 * Pick a trace file manually (fallback)
 */
export async function pickTraceFile(): Promise<File | null> {
  if (!isFileSystemAccessAPISupported()) {
    return null;
  }

  try {
    const [handle] = await window.showOpenFilePicker({
      types: [{
        description: 'Trace Data',
        accept: { 'application/jsonl': ['.jsonl', '.json'] }
      }],
      multiple: false
    });

    return await handle.getFile();
  } catch (e) {
    if ((e as Error).name !== 'AbortError') {
      console.error('Trace file picker error:', e);
    }
    return null;
  }
}
