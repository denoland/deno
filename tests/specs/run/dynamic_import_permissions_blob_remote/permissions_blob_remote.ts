const code = `import "https://example.com/some/file.ts";`;
const blob = new Blob([code]);
await import(URL.createObjectURL(blob));
