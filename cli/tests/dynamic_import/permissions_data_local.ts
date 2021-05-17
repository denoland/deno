// This file doesn't really exist, but it doesn't matter, a "PermissionsDenied" error should be thrown.
const code = `import "file:///local_file.ts";`;
await import(`data:application/javascript;base64,${btoa(code)}`);
