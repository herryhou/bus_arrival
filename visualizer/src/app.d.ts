// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}

	// File System Access API types
	interface Window {
		showOpenFilePicker(): Promise<FileSystemFileHandle[]>;
		showOpenFilePicker(options: {
			types?: Array<{
				description?: string;
				accept: Record<string, string[]>;
			}>;
			multiple?: boolean;
		}): Promise<FileSystemFileHandle[]>;
	}

	interface FileSystemFileHandle {
		getParent(): Promise<FileSystemDirectoryHandle>;
	}

	interface FileSystemDirectoryHandle {
		values(): AsyncIterableIterator<FileSystemHandle>;
	}
}

export {};
