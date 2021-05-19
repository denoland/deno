// This file doesn't really exist, but it doesn't matter, a "PermissionsDenied" error should be thrown.
const code = `import "https://example.com/some/file.ts";`;
const blob = new Blob([code]);
new Worker(URL.createObjectURL(blob), { type: "module" });
