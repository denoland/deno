// This file doesn't really exist, but it doesn't matter, a "PermissionsDenied" error should be thrown.
const code = `import "https://example.com/some/file.ts";`;
const blob = new Blob([code]);
await import(URL.createObjectURL(blob));
