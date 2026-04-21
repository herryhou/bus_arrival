import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  isFileSystemAccessAPISupported,
  extractBasename,
  generateTraceFilenames
} from './filePicker';

describe('filePicker', () => {
  describe('isFileSystemAccessAPISupported', () => {
    it('returns true when showOpenFilePicker is available', () => {
      vi.stubGlobal('showOpenFilePicker', vi.fn());
      expect(isFileSystemAccessAPISupported()).toBe(true);
      vi.unstubAllGlobals();
    });

    it('returns false when showOpenFilePicker is not available', () => {
      expect(isFileSystemAccessAPISupported()).toBe(false);
    });
  });

  describe('extractBasename', () => {
    it('extracts basename from filename with extension', () => {
      expect(extractBasename('route_data.bin')).toBe('route_data');
      expect(extractBasename('ty225_short_detour.bin')).toBe('ty225_short_detour');
    });

    it('returns full string if no extension', () => {
      expect(extractBasename('route_data')).toBe('route_data');
    });

    it('handles multiple dots', () => {
      expect(extractBasename('route.data.bin')).toBe('route.data');
    });
  });

  describe('generateTraceFilenames', () => {
    it('generates all expected filename variants', () => {
      const result = generateTraceFilenames('ty225_route');
      expect(result).toEqual([
        'ty225_route_trace.jsonl',
        'ty225_route.jsonl',
        'ty225_route_trace.json',
        'ty225_route.json'
      ]);
    });

    it('handles basename with underscores', () => {
      const result = generateTraceFilenames('ty225_short_detour');
      expect(result).toContain('ty225_short_detour_trace.jsonl');
    });
  });
});
