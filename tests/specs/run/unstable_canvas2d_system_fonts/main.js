// Copyright 2018-2026 the Deno authors. MIT license.

// System fonts only reach canvas text after `Deno.registerLocalFonts()`
// has passed its `--allow-sys=localFonts` check.

const ctx = new OffscreenCanvas(100, 30).getContext("2d");
ctx.font = "20px sans-serif";
console.log("before", ctx.measureText("Deno").width > 0);

try {
  await Deno.registerLocalFonts();
  console.log("registerLocalFonts", "ok");
} catch (e) {
  console.log("registerLocalFonts", e.name);
}

console.log("after", ctx.measureText("Deno").width > 0);
