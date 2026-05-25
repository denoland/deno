// Regression for https://github.com/denoland/deno/issues/34324 —
// `"brotli"` must be assignable to `CompressionFormat`.
const compression: CompressionStream = new CompressionStream("brotli");
const decompression: DecompressionStream = new DecompressionStream("brotli");
console.log(compression, decompression);
