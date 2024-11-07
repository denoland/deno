// This file doesn't really exist, but it doesn't matter, a "NotCapable" error should be thrown.
const code = `import "file:///${
  Deno.build.os == "windows" ? "C:/" : ""
}local_file.ts";`;
const blob = new Blob([code]);
await import(URL.createObjectURL(blob));
