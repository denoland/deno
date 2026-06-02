// Regression test for https://github.com/denoland/deno/issues/34324
// Ensure that "brotli" stays in `CompressionFormat` across TypeScript upgrades,
// in `lib.deno_web.d.ts`, `lib.dom.d.ts`, and `lib.webworker.d.ts`.
const _compress = new CompressionStream("brotli");
const _decompress = new DecompressionStream("brotli");
