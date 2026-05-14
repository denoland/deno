// Microbench for TextEncoder / TextDecoder.
//
// Covers: encode small ASCII, encode medium ASCII, encode UTF-8 mixed,
//         encodeInto small, decode short ASCII, decode medium ASCII,
//         decode UTF-8 mixed, decode stream chunks.

const ITERS_SMALL = 500_000;
const ITERS_MEDIUM = 50_000;

function bench(name, fn, iters = ITERS_SMALL) {
  for (let i = 0; i < 1000; i++) fn(i);
  const t0 = performance.now();
  for (let i = 0; i < iters; i++) fn(i);
  const t1 = performance.now();
  const ms = t1 - t0;
  const nsPerOp = (ms * 1e6) / iters;
  console.log(JSON.stringify({ name, ms: ms.toFixed(2), ns_per_op: nsPerOp.toFixed(1) }));
}

const enc = new TextEncoder();
const decUtf8 = new TextDecoder();
const decUtf8Fatal = new TextDecoder("utf-8", { fatal: true });

const tiny = "hello world!"; // 12 bytes ASCII
const small = "Content-Type: application/json"; // 30 bytes ASCII
const medium = "x".repeat(1000); // 1 KB ASCII
const large = "x".repeat(1 << 20); // 1 MB ASCII
const utf8mixed = "naïve résumé café — 日本語テキスト"; // multibyte
const utf8mediumMixed = ("naïve résumé café — 日本語テキスト ").repeat(20);

// 1: encode tiny ASCII
bench("encode_tiny_ascii", () => {
  enc.encode(tiny);
});

// 2: encode small ASCII
bench("encode_small_ascii", () => {
  enc.encode(small);
});

// 3: encode medium ASCII (1 KB)
bench("encode_medium_ascii", () => {
  enc.encode(medium);
}, ITERS_MEDIUM);

// 4: encode UTF-8 mixed
bench("encode_utf8_mixed", () => {
  enc.encode(utf8mixed);
});

// 5: encodeInto small (reuses dest)
const dest = new Uint8Array(64);
bench("encode_into_small", () => {
  enc.encodeInto(small, dest);
});

// Pre-encode for decode benches
const tinyBytes = enc.encode(tiny);
const smallBytes = enc.encode(small);
const mediumBytes = enc.encode(medium);
const largeBytes = enc.encode(large);
const utf8mixedBytes = enc.encode(utf8mixed);
const utf8mediumMixedBytes = enc.encode(utf8mediumMixed);

// 6: decode tiny ASCII
bench("decode_tiny_ascii", () => {
  decUtf8.decode(tinyBytes);
});

// 7: decode small ASCII
bench("decode_small_ascii", () => {
  decUtf8.decode(smallBytes);
});

// 8: decode medium ASCII (1 KB)
bench("decode_medium_ascii", () => {
  decUtf8.decode(mediumBytes);
}, ITERS_MEDIUM);

// 9: decode large ASCII (1 MB)
bench("decode_large_ascii_1mb", () => {
  decUtf8.decode(largeBytes);
}, 1_000);

// 10: decode UTF-8 mixed
bench("decode_utf8_mixed", () => {
  decUtf8.decode(utf8mixedBytes);
});

// 11: decode UTF-8 medium mixed (multi-byte path)
bench("decode_utf8_medium_mixed", () => {
  decUtf8.decode(utf8mediumMixedBytes);
}, ITERS_MEDIUM);

// 12: decode with stream:true (5 chunks)
const chunks = [smallBytes, smallBytes, smallBytes, smallBytes, smallBytes];
bench("decode_stream_5chunks", () => {
  const dec = new TextDecoder();
  let s = "";
  for (let j = 0; j < chunks.length - 1; j++) {
    s += dec.decode(chunks[j], { stream: true });
  }
  s += dec.decode(chunks[chunks.length - 1]);
}, ITERS_MEDIUM);

// 13: TextEncoder construct
bench("encoder_construct", () => {
  new TextEncoder();
});

// 14: TextDecoder construct (default utf-8)
bench("decoder_construct", () => {
  new TextDecoder();
}, ITERS_MEDIUM);
