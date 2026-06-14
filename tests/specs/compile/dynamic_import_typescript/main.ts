// A compiled binary can dynamically import TypeScript that it builds at
// runtime, via `data:` and `blob:` URLs, as well as a local TypeScript file
// that was not embedded at compile time. The embedded module loader transpiles
// the TypeScript source on the fly (see #27945).

const tsSource =
  `export const greet = (name: string): string => "hello " + name;`;

const dataUrl = "data:text/typescript;charset=utf-8," +
  encodeURIComponent(tsSource);
const fromData = await import(dataUrl);
console.log(fromData.greet("data"));

const blob = new Blob([tsSource], { type: "text/typescript" });
const blobUrl = URL.createObjectURL(blob);
const fromBlob = await import(blobUrl);
console.log(fromBlob.greet("blob"));
URL.revokeObjectURL(blobUrl);

// A TypeScript file discovered on disk at runtime. Its specifier is built from
// the real cwd (compiled binaries remap embedded paths into a virtual root, so
// the file must be addressed by its real on-disk path), and it is not part of
// the compile-time module graph.
const cwd = Deno.cwd().replaceAll("\\", "/");
const pluginUrl = `file://${cwd.startsWith("/") ? "" : "/"}${cwd}/plugin.ts`;
const fromFile = await import(pluginUrl);
console.log(fromFile.greet("file"));
