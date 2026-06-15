// Regression test for https://github.com/denoland/deno/issues/34324
// Ensure that "brotli" stays in `CompressionFormat` across TypeScript upgrades,
// in `lib.deno_web.d.ts`, `lib.dom.d.ts`, and `lib.webworker.d.ts`.
const _compress = new CompressionStream("brotli");
const _decompress = new DecompressionStream("brotli");

// Regression test for https://github.com/denoland/deno/issues/31878
// Default Uint8Array streams should pipe through compression streams.
declare const uint8ArrayStream: ReadableStream<Uint8Array>;
declare const uint8ArrayLikeStream: ReadableStream<
  Uint8Array<ArrayBufferLike>
>;
declare const uint8ArrayBufferStream: ReadableStream<Uint8Array<ArrayBuffer>>;

uint8ArrayStream.pipeThrough(new CompressionStream("gzip"));
uint8ArrayLikeStream.pipeThrough(new CompressionStream("gzip"));
uint8ArrayBufferStream.pipeThrough(new CompressionStream("gzip"));

uint8ArrayStream.pipeThrough(new DecompressionStream("gzip"));
uint8ArrayLikeStream.pipeThrough(new DecompressionStream("gzip"));
uint8ArrayBufferStream.pipeThrough(new DecompressionStream("gzip"));
