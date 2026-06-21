// Copyright 2018-2026 the Deno authors. MIT license.

import {
  assert,
  assertAlmostEquals,
  assertEquals,
  assertFalse,
  assertRejects,
  assertThrows,
} from "./test_util.ts";

let isCI: boolean;
try {
  isCI = (Deno.env.get("CI")?.length ?? 0) > 0;
} catch {
  isCI = true;
}

// Skip rendering tests on Linux CI (Vulkan emulator) and macOS x86 CI (no virtual GPU).
const isCIWithoutGPU = (Deno.build.os === "linux" ||
  (Deno.build.os === "darwin" && Deno.build.arch === "x86_64")) && isCI;
const isWsl = await checkIsWsl();

// Detect whether any canvas2d renderer (Gpu, Hybrid, or Cpu fallback) is functional.
const hasCanvasRenderer = await detectCanvasRenderer();

async function detectCanvasRenderer(): Promise<boolean> {
  const canvas = new OffscreenCanvas(1, 1);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "white";
  ctx.fillRect(0, 0, 1, 1);
  const blob = await canvas.convertToBlob({ type: "image/png" });
  const bitmap = await createImageBitmap(blob);
  // @ts-ignore: Deno[Deno.internal] allowed
  const pixels: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);
  return pixels[0] === 255;
}

// --- Context creation ---

Deno.test(function canvas2dGetContext() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d");
  assert(ctx !== null);
  assert(ctx instanceof OffscreenCanvasRenderingContext2D);
});

Deno.test(function canvas2dContextIsSticky() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx1 = canvas.getContext("2d");
  const ctx2 = canvas.getContext("2d");
  assertEquals(ctx1, ctx2);
});

Deno.test(function canvas2dContextExcludesOtherTypes() {
  const canvas = new OffscreenCanvas(10, 10);
  canvas.getContext("2d");
  assertEquals(canvas.getContext("bitmaprenderer"), null);
});

Deno.test(function canvas2dCanvasGetter() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  assertEquals(ctx.canvas, canvas);
});

// --- fillStyle ---

Deno.test(function canvas2dFillStyleDefault() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  assertEquals(ctx.fillStyle, "#000000");
});

Deno.test(function canvas2dFillStyleNamedColor() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "red";
  assertEquals(ctx.fillStyle, "#ff0000");
});

Deno.test(function canvas2dFillStyleHex() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#00ff00";
  assertEquals(ctx.fillStyle, "#00ff00");
});

Deno.test(function canvas2dFillStyleSemiTransparent() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "rgba(0, 0, 255, 1)";
  assertEquals(ctx.fillStyle, "#0000ff");
  ctx.fillStyle = "rgba(0, 0, 255, 0)";
  assertEquals(ctx.fillStyle, "rgba(0, 0, 255, 0)");
});

Deno.test(function canvas2dFillStyleInvalidIgnored() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "red";
  ctx.fillStyle = "not-a-color";
  assertEquals(ctx.fillStyle, "#ff0000");
});

// --- strokeStyle ---

Deno.test(function canvas2dStrokeStyleDefault() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  assertEquals(ctx.strokeStyle, "#000000");
});

Deno.test(function canvas2dStrokeStyleRoundTrip() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.strokeStyle = "blue";
  assertEquals(ctx.strokeStyle, "#0000ff");
});

Deno.test(function canvas2dStrokeStyleInvalidIgnored() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.strokeStyle = "blue";
  ctx.strokeStyle = "not-a-color";
  assertEquals(ctx.strokeStyle, "#0000ff");
});

// --- globalAlpha ---

Deno.test(function canvas2dGlobalAlphaDefault() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  assertEquals(ctx.globalAlpha, 1.0);
});

Deno.test(function canvas2dGlobalAlphaRoundTrip() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.globalAlpha = 0.5;
  assertEquals(ctx.globalAlpha, 0.5);
});

Deno.test(function canvas2dGlobalAlphaOutOfRangeIgnored() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.globalAlpha = 0.5;
  ctx.globalAlpha = 2.0;
  assertEquals(ctx.globalAlpha, 0.5);
  ctx.globalAlpha = -0.5;
  assertEquals(ctx.globalAlpha, 0.5);
  ctx.globalAlpha = Infinity;
  assertEquals(ctx.globalAlpha, 0.5);
  ctx.globalAlpha = NaN;
  assertEquals(ctx.globalAlpha, 0.5);
});

// --- font ---

Deno.test(function canvas2dFontDefault() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  assertEquals(ctx.font, "10px sans-serif");
});

Deno.test(function canvas2dFontRoundTrip() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.font = "16px serif";
  assertEquals(ctx.font, "16px serif");
});

Deno.test(function canvas2dFontBold() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.font = "bold 12px monospace";
  // bold → weight 700
  assertEquals(ctx.font, "700 12px monospace");
});

Deno.test(function canvas2dFontItalic() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.font = "italic 14px sans-serif";
  assertEquals(ctx.font, "italic 14px sans-serif");
});

Deno.test(function canvas2dFontInvalidIgnored() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.font = "16px serif";
  ctx.font = "not-a-font-string!@#";
  assertEquals(ctx.font, "16px serif");
});

// --- textAlign ---

Deno.test(function canvas2dTextAlignDefault() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  assertEquals(ctx.textAlign, "start");
});

Deno.test(function canvas2dTextAlignAllValues() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  for (const v of ["start", "end", "left", "right", "center"] as const) {
    ctx.textAlign = v;
    assertEquals(ctx.textAlign, v);
  }
});

Deno.test(function canvas2dTextAlignInvalidIgnored() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.textAlign = "center";
  // @ts-expect-error: testing invalid value
  ctx.textAlign = "invalid";
  assertEquals(ctx.textAlign, "center");
});

// --- textBaseline ---

Deno.test(function canvas2dTextBaselineDefault() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  assertEquals(ctx.textBaseline, "alphabetic");
});

Deno.test(function canvas2dTextBaselineAllValues() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  for (
    const v of [
      "top",
      "hanging",
      "middle",
      "alphabetic",
      "ideographic",
      "bottom",
    ] as const
  ) {
    ctx.textBaseline = v;
    assertEquals(ctx.textBaseline, v);
  }
});

Deno.test(function canvas2dTextBaselineInvalidIgnored() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.textBaseline = "middle";
  // @ts-expect-error: testing invalid value
  ctx.textBaseline = "invalid";
  assertEquals(ctx.textBaseline, "middle");
});

// --- measureText ---

Deno.test(function canvas2dMeasureTextReturnsTextMetrics() {
  const canvas = new OffscreenCanvas(100, 100);
  const ctx = canvas.getContext("2d")!;
  const m = ctx.measureText("Hello");
  // @ts-ignore: TextMetrics has no construct signature
  assert(m instanceof TextMetrics);
  assert(m.width >= 0);
  assert(typeof m.actualBoundingBoxLeft === "number");
  assert(typeof m.actualBoundingBoxRight === "number");
  assert(typeof m.fontBoundingBoxAscent === "number");
  assert(typeof m.fontBoundingBoxDescent === "number");
});

Deno.test(function canvas2dMeasureTextEmptyString() {
  const canvas = new OffscreenCanvas(100, 100);
  const ctx = canvas.getContext("2d")!;
  const m = ctx.measureText("");
  assertEquals(m.width, 0);
});

// --- CanvasRenderingContext2DSettings ---

Deno.test(function canvas2dSettingsDefault() {
  const canvas = new OffscreenCanvas(10, 10);
  // No options → must succeed with default alpha: true.
  assert(canvas.getContext("2d") !== null);
});

Deno.test(function canvas2dSettingsAlphaFalse() {
  const canvas = new OffscreenCanvas(10, 10);
  assert(canvas.getContext("2d", { alpha: false }) !== null);
});

Deno.test(function canvas2dSettingsColorSpaceSrgb() {
  const canvas = new OffscreenCanvas(10, 10);
  assert(canvas.getContext("2d", { colorSpace: "srgb" }) !== null);
});

Deno.test(function canvas2dSettingsColorSpaceDisplayP3() {
  const canvas = new OffscreenCanvas(10, 10);
  // display-p3 is accepted and stored; rendering parity is a TODO.
  assert(canvas.getContext("2d", { colorSpace: "display-p3" }) !== null);
});

Deno.test(function canvas2dSettingsWillReadFrequently() {
  const canvas = new OffscreenCanvas(10, 10);
  assert(canvas.getContext("2d", { willReadFrequently: true }) !== null);
});

Deno.test(function canvas2dSettingsDesynchronized() {
  const canvas = new OffscreenCanvas(10, 10);
  assert(canvas.getContext("2d", { desynchronized: true }) !== null);
});

// --- Phase 2: Paths (basic API, no pixel readback) ---

Deno.test(function canvas2dPathBasics() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.beginPath();
  ctx.moveTo(0, 0);
  ctx.lineTo(5, 5);
  ctx.rect(1, 1, 2, 2);
  ctx.closePath();
  ctx.strokeRect(0, 0, 1, 1);
  // Should not throw
  ctx.fill();
  ctx.stroke();
  ctx.clip();
});

Deno.test(function canvas2dPath2D() {
  const p = new Path2D();
  p.moveTo(0, 0);
  p.lineTo(10, 10);
  p.rect(0, 0, 4, 4);
  const p2 = new Path2D(p);
  // basic
  assert(p2 !== p);
});

// --- Rendering (GPU required) ---

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU || !hasCanvasRenderer,
}, async function canvas2dFillRectRendersPixel() {
  const canvas = new OffscreenCanvas(4, 4);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "rgb(255, 0, 0)";
  ctx.fillRect(0, 0, 4, 4);
  const blob = await canvas.convertToBlob({ type: "image/png" });
  const bitmap = await createImageBitmap(blob);
  // @ts-ignore: Deno[Deno.internal] allowed
  const pixels: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);
  // First pixel should be red (R=255, G=0, B=0, A=255).
  assertEquals(pixels[0], 255); // R
  assertEquals(pixels[1], 0); // G
  assertEquals(pixels[2], 0); // B
  assertEquals(pixels[3], 255); // A
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function canvas2dDefaultBackgroundIsTransparent() {
  const canvas = new OffscreenCanvas(2, 2);
  canvas.getContext("2d");
  const blob = await canvas.convertToBlob({ type: "image/png" });
  const bitmap = await createImageBitmap(blob);
  // @ts-ignore: Deno[Deno.internal] allowed
  const pixels: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);
  // Default alpha:true → blank canvas should be fully transparent.
  assertEquals(pixels[3], 0); // A of first pixel
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function canvas2dAlphaFalseBackgroundIsOpaqueBlack() {
  const canvas = new OffscreenCanvas(2, 2);
  canvas.getContext("2d", { alpha: false });
  const blob = await canvas.convertToBlob({ type: "image/png" });
  const bitmap = await createImageBitmap(blob);
  // @ts-ignore: Deno[Deno.internal] allowed
  const pixels: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);
  // alpha:false → blank canvas should be opaque black.
  assertEquals(pixels[0], 0); // R
  assertEquals(pixels[1], 0); // G
  assertEquals(pixels[2], 0); // B
  assertEquals(pixels[3], 255); // A
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU,
}, async function canvas2dResizeResetsScene() {
  const canvas = new OffscreenCanvas(4, 4);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "rgb(255, 0, 0)";
  ctx.fillRect(0, 0, 4, 4);
  // Resize clears the accumulated scene.
  canvas.width = 4;
  const blob = await canvas.convertToBlob({ type: "image/png" });
  const bitmap = await createImageBitmap(blob);
  // @ts-ignore: Deno[Deno.internal] allowed
  const pixels: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);
  // After resize, canvas should be blank (transparent).
  assertEquals(pixels[3], 0);
});

// --- Text rendering ---

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU || !hasCanvasRenderer,
}, async function canvas2dFillTextRendersGlyphs() {
  const canvas = new OffscreenCanvas(100, 30);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "white";
  ctx.fillRect(0, 0, 100, 30);
  ctx.fillStyle = "black";
  ctx.font = "20px sans-serif";
  ctx.textBaseline = "top";
  ctx.fillText("Deno", 5, 5);
  const blob = await canvas.convertToBlob({ type: "image/png" });
  const bitmap = await createImageBitmap(blob);
  // @ts-ignore: Deno[Deno.internal] allowed
  const pixels: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);
  let hasNonWhite = false;
  for (let i = 0; i < pixels.length; i += 4) {
    if (pixels[i] < 255) {
      hasNonWhite = true;
      break;
    }
  }
  assert(hasNonWhite, "fillText should render visible glyphs");
});

Deno.test({
  permissions: { read: true, env: true },
  ignore: isWsl || isCIWithoutGPU || !hasCanvasRenderer,
}, async function canvas2dFillTextCustomFontCJK() {
  const fontData = await Deno.readFile(
    "tests/testdata/NotoSerifCJKjp-Regular-subset.otf",
  );
  const face = new FontFace("NotoSerifCJKjp", fontData.buffer);
  await face.load();
  Deno.fonts.add(face);
  try {
    const canvas = new OffscreenCanvas(200, 50);
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = "white";
    ctx.fillRect(0, 0, 200, 50);
    ctx.fillStyle = "black";
    ctx.font = "30px 'NotoSerifCJKjp'";
    ctx.textBaseline = "top";
    ctx.fillText("こんにちは", 5, 5);
    const blob = await canvas.convertToBlob({ type: "image/png" });
    const bitmap = await createImageBitmap(blob);
    // @ts-ignore: Deno[Deno.internal] allowed
    const pixels: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);
    let hasNonWhite = false;
    for (let i = 0; i < pixels.length; i += 4) {
      if (pixels[i] < 255) {
        hasNonWhite = true;
        break;
      }
    }
    assert(
      hasNonWhite,
      "CJK text with custom font should render visible glyphs",
    );
  } finally {
    Deno.fonts.delete(face);
  }
});

// --- CanvasTextDrawingStyles new properties ---

Deno.test(function canvas2dDirectionDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.direction, "inherit");
});

Deno.test(function canvas2dDirectionRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.direction = "rtl";
  assertEquals(ctx.direction, "rtl");
  ctx.direction = "ltr";
  assertEquals(ctx.direction, "ltr");
});

Deno.test(function canvas2dDirectionInvalidIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.direction = "invalid" as CanvasDirection;
  assertEquals(ctx.direction, "inherit");
});

Deno.test(function canvas2dDirectionInheritReset() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.direction = "rtl";
  ctx.direction = "inherit";
  assertEquals(ctx.direction, "inherit");
});

Deno.test(function canvas2dFontKerningDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.fontKerning, "auto");
});

Deno.test(function canvas2dFontKerningRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.fontKerning = "none";
  assertEquals(ctx.fontKerning, "none");
  ctx.fontKerning = "normal";
  assertEquals(ctx.fontKerning, "normal");
  ctx.fontKerning = "auto";
  assertEquals(ctx.fontKerning, "auto");
});

Deno.test(function canvas2dFontKerningInvalidIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.fontKerning = "none";
  ctx.fontKerning = "invalid" as CanvasFontKerning;
  assertEquals(ctx.fontKerning, "none");
});

Deno.test(function canvas2dFontStretchDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.fontStretch, "normal");
});

Deno.test(function canvas2dFontStretchRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  for (
    const v of [
      "ultra-condensed",
      "extra-condensed",
      "condensed",
      "semi-condensed",
      "normal",
      "semi-expanded",
      "expanded",
      "extra-expanded",
      "ultra-expanded",
    ] as const
  ) {
    ctx.fontStretch = v;
    assertEquals(ctx.fontStretch, v);
  }
});

Deno.test(function canvas2dFontStretchInvalidIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.fontStretch = "condensed";
  ctx.fontStretch = "invalid" as CanvasFontStretch;
  assertEquals(ctx.fontStretch, "condensed");
});

Deno.test(function canvas2dFontVariantCapsDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.fontVariantCaps, "normal");
});

Deno.test(function canvas2dFontVariantCapsRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  for (
    const v of [
      "normal",
      "small-caps",
      "all-small-caps",
      "petite-caps",
      "all-petite-caps",
      "unicase",
      "titling-caps",
    ] as const
  ) {
    ctx.fontVariantCaps = v;
    assertEquals(ctx.fontVariantCaps, v);
  }
});

Deno.test(function canvas2dFontVariantCapsInvalidIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.fontVariantCaps = "small-caps";
  ctx.fontVariantCaps = "invalid" as CanvasFontVariantCaps;
  assertEquals(ctx.fontVariantCaps, "small-caps");
});

Deno.test(function canvas2dLetterSpacingDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.letterSpacing, "0px");
});

Deno.test(function canvas2dLetterSpacingRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.letterSpacing = "2px";
  assertEquals(ctx.letterSpacing, "2px");
  ctx.letterSpacing = "0px";
  assertEquals(ctx.letterSpacing, "0px");
});

Deno.test(function canvas2dLetterSpacingInvalidIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.letterSpacing = "2px";
  ctx.letterSpacing = "not-a-length";
  assertEquals(ctx.letterSpacing, "2px");
});

Deno.test(function canvas2dWordSpacingDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.wordSpacing, "0px");
});

Deno.test(function canvas2dWordSpacingRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.wordSpacing = "4px";
  assertEquals(ctx.wordSpacing, "4px");
  ctx.wordSpacing = "0px";
  assertEquals(ctx.wordSpacing, "0px");
});

Deno.test(function canvas2dWordSpacingInvalidIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.wordSpacing = "4px";
  ctx.wordSpacing = "not-a-length";
  assertEquals(ctx.wordSpacing, "4px");
});

Deno.test(function canvas2dTextRenderingDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.textRendering, "auto");
});

Deno.test(function canvas2dTextRenderingRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  for (
    const v of [
      "auto",
      "optimizeSpeed",
      "optimizeLegibility",
      "geometricPrecision",
    ] as const
  ) {
    ctx.textRendering = v;
    assertEquals(ctx.textRendering, v);
  }
});

Deno.test(function canvas2dTextRenderingInvalidIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.textRendering = "optimizeLegibility";
  ctx.textRendering = "invalid" as CanvasTextRendering;
  assertEquals(ctx.textRendering, "optimizeLegibility");
});

// --- FontFace constructor ---

Deno.test(function fontFaceConstructorRequiresArgs() {
  // @ts-expect-error: testing required-arg behavior
  assertThrows(() => new FontFace(), TypeError);
  // @ts-expect-error: testing required-arg behavior
  assertThrows(() => new FontFace("TestFont"), TypeError);
});

Deno.test(function fontFaceConstructorRequiresBufferSource() {
  // String sources are treated as URLs, which are explicitly unsupported.
  // @ts-expect-error: testing wrong source type
  assertThrows(() => new FontFace("TestFont", "not-a-buffer"), DOMException);
  // @ts-expect-error: testing wrong source type
  assertThrows(() => new FontFace("TestFont", 42), TypeError);
});

Deno.test(function fontFaceConstructorAcceptsArrayBuffer() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  assertEquals(face.family, "TestFont");
  assertEquals(face.status, "unloaded");
});

Deno.test(function fontFaceConstructorAcceptsTypedArray() {
  const face = new FontFace("TestFont", new Uint8Array([0, 1, 2, 3]));
  assertEquals(face.family, "TestFont");
});

Deno.test(function fontFaceDefaultDescriptors() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  assertEquals(face.style, "normal");
  assertEquals(face.weight, "normal");
  assertEquals(face.stretch, "normal");
  assertEquals(face.unicodeRange, "U+0-10FFFF");
  assertEquals(face.featureSettings, "normal");
  assertEquals(face.variationSettings, "normal");
  assertEquals(face.display, "auto");
  assertEquals(face.ascentOverride, "normal");
  assertEquals(face.descentOverride, "normal");
  assertEquals(face.lineGapOverride, "normal");
});

Deno.test(function fontFaceDescriptorsRoundTrip() {
  const face = new FontFace("TestFont", new ArrayBuffer(4), {
    style: "italic",
    weight: "700",
    stretch: "condensed",
    unicodeRange: "U+0025-00FF",
    featureSettings: '"smcp"',
    variationSettings: '"wght" 400',
    display: "swap",
    ascentOverride: "100%",
    descentOverride: "50%",
    lineGapOverride: "0%",
  });
  assertEquals(face.style, "italic");
  assertEquals(face.weight, "700");
  assertEquals(face.stretch, "condensed");
  assertEquals(face.unicodeRange, "U+0025-00FF");
  assertEquals(face.featureSettings, '"smcp"');
  assertEquals(face.variationSettings, '"wght" 400');
  assertEquals(face.display, "swap");
  assertEquals(face.ascentOverride, "100%");
  assertEquals(face.descentOverride, "50%");
  assertEquals(face.lineGapOverride, "0%");
});

Deno.test(function fontFaceConstructorThrowsOnInvalidDescriptors() {
  assertThrows(
    () =>
      new FontFace("TestFont", new ArrayBuffer(4), { style: "invalid-style" }),
    DOMException,
  );
  assertThrows(
    () => new FontFace("TestFont", new ArrayBuffer(4), { weight: "0" }),
    DOMException,
  );
  assertThrows(
    () =>
      new FontFace("TestFont", new ArrayBuffer(4), {
        stretch: "invalid-stretch",
      }),
    DOMException,
  );
});

// --- FontFace property setters ---

Deno.test(function fontFaceFamilySetter() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.family = "NewName";
  assertEquals(face.family, "NewName");
});

Deno.test(function fontFaceStyleSetter() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.style = "italic";
  assertEquals(face.style, "italic");
  face.style = "oblique";
  assertEquals(face.style, "oblique");
  face.style = "normal";
  assertEquals(face.style, "normal");
});

Deno.test(function fontFaceStyleSetterThrowsOnInvalid() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.style = "italic";
  assertThrows(() => {
    face.style = "invalid";
  }, DOMException);
  assertEquals(face.style, "italic");
});

Deno.test(function fontFaceWeightSetter() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.weight = "bold";
  assertEquals(face.weight, "bold");
  face.weight = "700";
  assertEquals(face.weight, "700");
  face.weight = "1000";
  assertEquals(face.weight, "1000");
});

Deno.test(function fontFaceWeightSetterThrowsOnOutOfRange() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.weight = "700";
  assertThrows(() => {
    face.weight = "0";
  }, DOMException);
  assertEquals(face.weight, "700");
  assertThrows(() => {
    face.weight = "1001";
  }, DOMException);
  assertEquals(face.weight, "700");
});

Deno.test(function fontFaceStretchSetter() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.stretch = "condensed";
  assertEquals(face.stretch, "condensed");
  face.stretch = "expanded";
  assertEquals(face.stretch, "expanded");
  face.stretch = "normal";
  assertEquals(face.stretch, "normal");
});

Deno.test(function fontFaceStretchSetterThrowsOnInvalid() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.stretch = "condensed";
  assertThrows(() => {
    face.stretch = "invalid";
  }, DOMException);
  assertEquals(face.stretch, "condensed");
});

// --- FontFace status / load ---

Deno.test(function fontFaceStatusInitiallyUnloaded() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  assertEquals(face.status, "unloaded");
});

Deno.test(async function fontFaceStatusTransitionsToLoadingOnLoad() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  // Trigger the load: status transitions immediately, rejection consumed below.
  const p = face.load();
  assertEquals(face.status, "loading");
  await p.catch(() => {});
});

Deno.test(async function fontFaceLoadRejectsOnInvalidBytes() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  await assertRejects(() => face.load(), Error);
  assertEquals(face.status, "error");
});

Deno.test(async function fontFaceLoadReturnsSamePromise() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  const p1 = face.load();
  const p2 = face.load();
  assertEquals(p1, p2);
  // Consume the rejection to avoid unhandled rejection.
  await p1.catch(() => {});
});

Deno.test(async function fontFaceLoadedGetterRejects() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  await assertRejects(() => face.loaded, Error);
});

// --- FontFaceSetLoadEvent ---

Deno.test(function fontFaceSetLoadEventFontfaces() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  const ev = new FontFaceSetLoadEvent("loadingdone", { fontfaces: [face] });
  assertEquals(ev.type, "loadingdone");
  assertEquals(ev.fontfaces.length, 1);
  assertEquals(ev.fontfaces[0], face);
});

Deno.test(function fontFaceSetLoadEventDefaultFontfaces() {
  const ev = new FontFaceSetLoadEvent("loading");
  assertEquals(ev.fontfaces.length, 0);
});

// --- FontFaceSet ---

Deno.test(function fontFaceSetIllegalConstructor() {
  // @ts-expect-error: testing illegal constructor
  assertThrows(() => new FontFaceSet(), TypeError);
});

Deno.test(function fontFaceSetDenoFontsExists() {
  // @ts-ignore: FontFaceSet has no construct signature
  assert(Deno.fonts instanceof FontFaceSet);
});

Deno.test(function fontFaceSetHasAndDelete() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  const set = Deno.fonts;
  assert(!set.has(face));
  set.add(face);
  assert(set.has(face));
  assert(set.delete(face));
  assert(!set.has(face));
});

Deno.test(function fontFaceSetSize() {
  const face1 = new FontFace("FontA", new ArrayBuffer(4));
  const face2 = new FontFace("FontB", new ArrayBuffer(4));
  const initialSize = Deno.fonts.size;
  Deno.fonts.add(face1);
  Deno.fonts.add(face2);
  assertEquals(Deno.fonts.size, initialSize + 2);
  Deno.fonts.delete(face1);
  Deno.fonts.delete(face2);
  assertEquals(Deno.fonts.size, initialSize);
});

Deno.test(function fontFaceSetAddReturnsSelf() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  const result = Deno.fonts.add(face);
  assertEquals(result, Deno.fonts);
  Deno.fonts.delete(face);
});

Deno.test(function fontFaceSetAddNonFontFaceThrows() {
  assertThrows(() => {
    // @ts-expect-error: testing wrong type
    Deno.fonts.add("not-a-fontface");
  }, TypeError);
});

Deno.test(function fontFaceSetDeleteAbsentReturnsFalse() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  assertEquals(Deno.fonts.delete(face), false);
});

Deno.test(async function fontFaceSetReadyResolvesWhenIdle() {
  // Remove any pending faces first.
  Deno.fonts.clear();
  const result = await Deno.fonts.ready;
  assertEquals(result, Deno.fonts);
});

Deno.test(function fontFaceSetStatusLoadedWhenIdle() {
  Deno.fonts.clear();
  assertEquals(Deno.fonts.status, "loaded");
});

Deno.test(function fontFaceSetDispatchesLoadingEvent() {
  const events: string[] = [];
  const onLoading = () => events.push("loading");
  Deno.fonts.addEventListener("loading", onLoading);

  const face = new FontFace("TestFont", new ArrayBuffer(4));
  Deno.fonts.add(face);

  assert(events.includes("loading"));
  Deno.fonts.removeEventListener("loading", onLoading);
  Deno.fonts.delete(face);
});

// --- FontFaceSet.check / load ---

// Generic families are always considered loaded (no font file needed).
Deno.test(function fontFaceSetCheckGenericFamilyReturnsTrue() {
  assert(Deno.fonts.check("12px sans-serif"));
  assert(Deno.fonts.check("12px serif"));
  assert(Deno.fonts.check("12px monospace"));
});

// Unknown custom font (not in the set) returns false.
Deno.test(function fontFaceSetCheckUnloadedFontReturnsFalse() {
  assertFalse(Deno.fonts.check("12px NonExistentCustomFont"));
});

// load() resolves with empty array when no font matches.
Deno.test(async function fontFaceSetLoadNoMatchResolvesEmpty() {
  const result = await Deno.fonts.load("12px NonExistentCustomFont");
  assertEquals(result.length, 0);
});

// check() throws SyntaxError on invalid font strings.
Deno.test(function fontFaceSetCheckThrowsOnSyntaxError() {
  assertThrows(() => Deno.fonts.check("menu"), DOMException);
  assertThrows(() => Deno.fonts.check("not-a-font-string"), DOMException);
});

// load() rejects with SyntaxError on invalid font strings.
Deno.test(async function fontFaceSetLoadRejectsOnSyntaxError() {
  await assertRejects(() => Deno.fonts.load("menu"), DOMException);
  // Comma-separated fallback list is forbidden.
  await assertRejects(
    () => Deno.fonts.load("12px Arial, sans-serif"),
    DOMException,
  );
});

// Real font: user-specified unicodeRange override.
Deno.test(
  { permissions: { read: true } },
  async function fontFaceSetUserSpecifiedUnicodeRange() {
    const bytes = await Deno.readFile(
      new URL(
        "../testdata/NotoSansCJKjp-Regular-subset-halt-min.otf",
        import.meta.url,
      ),
    );
    // User declares this face covers only U+4E00-9FFF regardless of actual coverage.
    const face = new FontFace("NotoSansCJK", bytes, {
      unicodeRange: "U+4E00-9FFF",
    });
    Deno.fonts.add(face);
    try {
      // Returns false before the font finishes loading.
      assertFalse(Deno.fonts.check("12px NotoSansCJK", "日"));
      await Deno.fonts.ready;

      // Returns true once loaded for CJK text (U+65E5 is in U+4E00-9FFF).
      assert(Deno.fonts.check("12px NotoSansCJK", "日"));
      // ASCII not covered by unicodeRange — face not needed → vacuously true.
      assert(Deno.fonts.check("12px NotoSansCJK", "A"));
      // Bold variant not in set → false.
      assertFalse(Deno.fonts.check("bold 12px NotoSansCJK", "日"));
      // Unknown family → false.
      assertFalse(Deno.fonts.check("12px NonExistentCustomFont", "日"));

      // load() returns the face for covered CJK text.
      const loaded = await Deno.fonts.load("12px NotoSansCJK", "日");
      assertEquals(loaded.length, 1);
      assertEquals(loaded[0].family, "NotoSansCJK");
      assertEquals(loaded[0].status, "loaded");

      // load() returns empty array for ASCII (not in U+4E00-9FFF).
      const none = await Deno.fonts.load("12px NotoSansCJK", "A");
      assertEquals(none.length, 0);
    } finally {
      Deno.fonts.delete(face);
    }
  },
);

// Real font: no unicodeRange specified — uses actual font file coverage.
// This subset covers ASCII (U+0020-U+007E) and U+56FD (国), but not U+65E5 (日).
Deno.test(
  { permissions: { read: true } },
  async function fontFaceSetFontFileCoverage() {
    const bytes = await Deno.readFile(
      new URL(
        "../testdata/NotoSansCJKjp-Regular-subset-halt-min.otf",
        import.meta.url,
      ),
    );
    const face = new FontFace("NotoSansCJK", bytes); // no unicodeRange
    Deno.fonts.add(face);
    try {
      await Deno.fonts.ready;
      // Font covers U+56FD (国) → load() returns the face.
      const loaded = await Deno.fonts.load("12px NotoSansCJK", "国");
      assertEquals(loaded.length, 1);
      // Font also covers ASCII → load() returns the face.
      const loadedA = await Deno.fonts.load("12px NotoSansCJK", "A");
      assertEquals(loadedA.length, 1);
      // Font does not cover U+65E5 (日) → load() returns empty.
      const none = await Deno.fonts.load("12px NotoSansCJK", "日");
      assertEquals(none.length, 0);
    } finally {
      Deno.fonts.delete(face);
    }
  },
);

async function checkIsWsl() {
  return Deno.build.os === "linux" && await hasMicrosoftProcVersion();

  async function hasMicrosoftProcVersion() {
    try {
      const procVersion = await Deno.readTextFile("/proc/version");
      return /microsoft/i.test(procVersion);
    } catch {
      return false;
    }
  }
}

Deno.test(
  { permissions: { sys: ["systemFonts"] } },
  async function loadSystemFontsSucceeds() {
    await Deno.loadSystemFonts();
  },
);

Deno.test(
  { permissions: { sys: [] } },
  async function loadSystemFontsRequiresPermission() {
    await assertRejects(
      () => Deno.loadSystemFonts(),
      Deno.errors.NotCapable,
    );
  },
);

// CanvasState tests

Deno.test(function canvas2dSaveRestorePreservesAndRestoresState() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.fillStyle = "red";
  ctx.save();
  ctx.fillStyle = "blue";
  assertEquals(ctx.fillStyle, "#0000ff");
  ctx.restore();
  assertEquals(ctx.fillStyle, "#ff0000");
});

Deno.test(function canvas2dRestoreOnEmptyStackIsNoOp() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  // Should not throw when the state stack is empty
  ctx.restore();
});

Deno.test(function canvas2dResetClearsStateToDefaults() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.fillStyle = "red";
  ctx.reset();
  assertEquals(ctx.fillStyle, "#000000");
});

Deno.test(function canvas2dIsContextLostReturnsFalse() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertFalse(ctx.isContextLost());
});

// CanvasTransform tests

Deno.test(function canvas2dGetTransformReturnsIdentityByDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const m = ctx.getTransform();
  assertEquals(m.a, 1);
  assertEquals(m.b, 0);
  assertEquals(m.c, 0);
  assertEquals(m.d, 1);
  assertEquals(m.e, 0);
  assertEquals(m.f, 0);
});

Deno.test(function canvas2dTranslateModifiesTransform() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.translate(10, 20);
  const m = ctx.getTransform();
  assertEquals(m.e, 10);
  assertEquals(m.f, 20);
});

Deno.test(function canvas2dScaleModifiesTransform() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.scale(2, 3);
  const m = ctx.getTransform();
  assertEquals(m.a, 2);
  assertEquals(m.d, 3);
});

Deno.test(function canvas2dRotateModifiesTransform() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.rotate(Math.PI / 2);
  const m = ctx.getTransform();
  assertAlmostEquals(m.a, 0, 1e-10);
  assertAlmostEquals(m.b, 1, 1e-10);
  assertAlmostEquals(m.c, -1, 1e-10);
  assertAlmostEquals(m.d, 0, 1e-10);
});

Deno.test(function canvas2dSetTransformSetsMatrixDirectly() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.setTransform(2, 0, 0, 3, 10, 20);
  const m = ctx.getTransform();
  assertEquals(m.a, 2);
  assertEquals(m.b, 0);
  assertEquals(m.c, 0);
  assertEquals(m.d, 3);
  assertEquals(m.e, 10);
  assertEquals(m.f, 20);
});

Deno.test(function canvas2dResetTransformResetsToIdentity() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.translate(10, 20);
  ctx.resetTransform();
  const m = ctx.getTransform();
  assertEquals(m.a, 1);
  assertEquals(m.b, 0);
  assertEquals(m.c, 0);
  assertEquals(m.d, 1);
  assertEquals(m.e, 0);
  assertEquals(m.f, 0);
});

Deno.test(function canvas2dSaveRestorePreservesTransform() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.translate(10, 20);
  ctx.save();
  ctx.translate(5, 5);
  const mAfterSecondTranslate = ctx.getTransform();
  assertEquals(mAfterSecondTranslate.e, 15);
  assertEquals(mAfterSecondTranslate.f, 25);
  ctx.restore();
  const mAfterRestore = ctx.getTransform();
  assertEquals(mAfterRestore.e, 10);
  assertEquals(mAfterRestore.f, 20);
});

Deno.test(function canvas2dNonFiniteTransformArgumentsAreIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;

  // translate with NaN — should be a no-op
  ctx.translate(NaN, 0);
  let m = ctx.getTransform();
  assertEquals(m.e, 0);
  assertEquals(m.f, 0);

  // scale with Infinity — should be a no-op
  ctx.scale(Infinity, 1);
  m = ctx.getTransform();
  assertEquals(m.a, 1);
  assertEquals(m.d, 1);

  // rotate with NaN — should be a no-op
  ctx.rotate(NaN);
  m = ctx.getTransform();
  assertEquals(m.a, 1);
  assertEquals(m.b, 0);
  assertEquals(m.c, 0);
  assertEquals(m.d, 1);
});

// --- Phase 2: Path / Vector Engine ---

Deno.test(function canvas2dBeginPathAndRect() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.beginPath();
  ctx.rect(0, 0, 5, 5);
  ctx.closePath();
  // No error
});

Deno.test(function canvas2dArcNegativeRadiusThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(
    () => ctx.arc(0, 0, -1, 0, 0),
    DOMException,
  );
});

Deno.test(function canvas2dArcToNegativeRadiusThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(
    () => ctx.arcTo(0, 0, 5, 5, -1),
    DOMException,
  );
});

Deno.test(function canvas2dEllipseNegativeRadiusThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(
    () => ctx.ellipse(5, 5, -1, 5, 0, 0, Math.PI * 2),
    DOMException,
  );
  assertThrows(
    () => ctx.ellipse(5, 5, 5, -1, 0, 0, Math.PI * 2),
    DOMException,
  );
});

Deno.test(function canvas2dArcNonFiniteArgumentsIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.arc(NaN, 0, 1, 0, Math.PI);
  ctx.arc(0, Infinity, 1, 0, Math.PI);
  // No error, no-op
});

Deno.test(function canvas2dMoveToLineToNonFiniteIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.beginPath();
  ctx.moveTo(NaN, 0);
  ctx.lineTo(0, Infinity);
  // No error
});

Deno.test(function canvas2dSetLineDashInvalidSilentlyReturns() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.setLineDash([5, 3]);
  assertEquals(ctx.getLineDash(), [5, 3]);

  // Negative values: silently ignored
  ctx.setLineDash([-1, 3]);
  assertEquals(ctx.getLineDash(), [5, 3]);

  // NaN: silently ignored
  ctx.setLineDash([NaN]);
  assertEquals(ctx.getLineDash(), [5, 3]);

  // Infinity: silently ignored
  ctx.setLineDash([Infinity]);
  assertEquals(ctx.getLineDash(), [5, 3]);
});

Deno.test(function canvas2dSetLineDashOddLengthDoubles() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.setLineDash([5, 3, 1]);
  assertEquals(ctx.getLineDash(), [5, 3, 1, 5, 3, 1]);
});

Deno.test(function canvas2dFillStrokeClipNoError() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.beginPath();
  ctx.rect(0, 0, 5, 5);
  ctx.fill();
  ctx.stroke();
  ctx.clip();
  // No error
});

Deno.test(function canvas2dFillWithFillRule() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.beginPath();
  ctx.rect(0, 0, 5, 5);
  ctx.fill("evenodd");
  ctx.fill("nonzero");
  // No error
});

Deno.test(function canvas2dStrokeRect() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.strokeRect(0, 0, 5, 5);
  // No error
});

Deno.test(function canvas2dPath2DConstructor() {
  const _p = new Path2D();
  _p.rect(0, 0, 10, 10);

  const _p2 = new Path2D(_p);
});

Deno.test(function canvas2dPath2DSvgPath() {
  const _p = new Path2D("M10 10 L20 20 Z");
});

Deno.test(function canvas2dPath2DArcNegativeRadiusThrows() {
  const p = new Path2D();
  assertThrows(
    () => p.arc(0, 0, -1, 0, 0),
    DOMException,
  );
});

Deno.test(function canvas2dPath2DAddPath() {
  const p1 = new Path2D();
  p1.rect(0, 0, 5, 5);
  const p2 = new Path2D();
  p2.addPath(p1);
  // No error
});

Deno.test(function canvas2dFillWithPath2D() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const p = new Path2D();
  p.rect(0, 0, 5, 5);
  ctx.fill(p);
  ctx.fill(p, "evenodd");
  ctx.stroke(p);
  // No error
});

Deno.test(function canvas2dGetImageDataBasic() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "rgb(255, 0, 0)";
  ctx.fillRect(0, 0, 2, 2);
  const data = ctx.getImageData(0, 0, 2, 2);
  assertEquals(data.width, 2);
  assertEquals(data.height, 2);
  assertEquals(data.data.length, 16);
});

Deno.test(function canvas2dGetImageDataZeroSizeThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(
    () => ctx.getImageData(0, 0, 0, 1),
    DOMException,
  );
  assertThrows(
    () => ctx.getImageData(0, 0, 1, 0),
    DOMException,
  );
});

Deno.test(function canvas2dIsPointInPath() {
  const ctx = new OffscreenCanvas(100, 100).getContext("2d")!;
  ctx.beginPath();
  ctx.rect(10, 10, 50, 50);
  assertEquals(ctx.isPointInPath(25, 25), true);
  assertEquals(ctx.isPointInPath(0, 0), false);
});

Deno.test(function canvas2dIsPointInPathWithPath2D() {
  const ctx = new OffscreenCanvas(100, 100).getContext("2d")!;
  const path = new Path2D();
  path.rect(10, 10, 50, 50);
  assertEquals(ctx.isPointInPath(path, 25, 25), true);
  assertEquals(ctx.isPointInPath(path, 0, 0), false);
});

Deno.test(function canvas2dIsPointInPathEvenOdd() {
  const ctx = new OffscreenCanvas(200, 200).getContext("2d")!;
  const path = new Path2D();
  path.rect(0, 0, 100, 100);
  path.rect(25, 25, 50, 50);
  assertEquals(ctx.isPointInPath(path, 50, 50), true);
  assertEquals(ctx.isPointInPath(path, 50, 50, "nonzero"), true);
  assertEquals(ctx.isPointInPath(path, 50, 50, "evenodd"), false);
});

Deno.test(function canvas2dIsPointInPathInvalidFillRule() {
  const ctx = new OffscreenCanvas(100, 100).getContext("2d")!;
  assertThrows(() => ctx.isPointInPath(50, 50, "gazonk"), TypeError);
  const path = new Path2D();
  path.rect(0, 0, 100, 100);
  assertThrows(() => ctx.isPointInPath(path, 50, 50, "gazonk"), TypeError);
});

Deno.test(function canvas2dIsPointInPathInvalidFirstArg() {
  const ctx = new OffscreenCanvas(100, 100).getContext("2d")!;
  assertThrows(
    () => ctx.isPointInPath(null as unknown as Path2D, 50, 50),
    TypeError,
  );
  assertThrows(
    () => ctx.isPointInPath(undefined as unknown as Path2D, 50, 50),
    TypeError,
  );
});

Deno.test(function canvas2dIsPointInStrokeWithPath2D() {
  const ctx = new OffscreenCanvas(100, 100).getContext("2d")!;
  const path = new Path2D();
  path.rect(20, 20, 60, 60);
  assertEquals(ctx.isPointInStroke(path, 20, 20), true);
  assertEquals(ctx.isPointInStroke(path, 50, 50), false);
});
