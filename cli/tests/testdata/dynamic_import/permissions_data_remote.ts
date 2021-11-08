// This file doesn't really exist, but it doesn't matter, a "PermissionsDenied" error should be thrown.
const code = `import "https://example.com/some/file.ts";`;
await import(`data:application/javascript;base64,${btoa(code)}`);
