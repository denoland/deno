// Copyright 2018-2026 the Deno authors. MIT license.

// System fonts only reach canvas text after `Deno.registerAllLocalFonts()`
// has passed its `--allow-sys=localFonts` check.

const ctx = new OffscreenCanvas(100, 30).getContext("2d");
ctx.font = "20px sans-serif";
console.log("before", ctx.measureText("Deno").width > 0);

try {
  await Deno.registerAllLocalFonts();
  console.log("registerAllLocalFonts", "ok");
} catch (e) {
  console.log("registerAllLocalFonts", e.name);
}

console.log("after", ctx.measureText("Deno").width > 0);
