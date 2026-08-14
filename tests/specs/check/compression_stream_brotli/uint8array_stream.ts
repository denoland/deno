// Regression test for https://github.com/denoland/deno/issues/31878
// Default Uint8Array streams should pipe through compression streams in Deno's
// default web lib.
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
