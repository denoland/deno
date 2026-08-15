// Copyright 2018-2026 the Deno authors. MIT license.

import {
  assert,
  assertAlmostEquals,
  assertEquals,
  assertFalse,
  assertRejects,
  assertStrictEquals,
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

// Detect whether any canvas2d renderer (Gpu or Cpu fallback) is functional.
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

Deno.test(function canvas2dFillStyleModernSyntax() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  // Modern space-separated rgb() still serializes in the legacy form.
  ctx.fillStyle = "rgb(255 0 0)";
  assertEquals(ctx.fillStyle, "#ff0000");
  ctx.fillStyle = "rgb(100% 0% 0% / 50%)";
  assertEquals(ctx.fillStyle, "rgba(255, 0, 0, 0.5)");
  ctx.fillStyle = "hsl(120 100% 50%)";
  assertEquals(ctx.fillStyle, "#00ff00");
  ctx.fillStyle = "lab(50 0 0)";
  assertEquals(ctx.fillStyle, "lab(50 0 0)");
  ctx.fillStyle = "oklch(0.5 0.2 120 / 0.5)";
  assertEquals(ctx.fillStyle, "oklch(0.5 0.2 120 / 0.5)");
  ctx.fillStyle = "color(display-p3 1 0 0)";
  assertEquals(ctx.fillStyle, "color(display-p3 1 0 0)");
});

Deno.test(function canvas2dFillStyleCalc() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "rgb(calc(200 + 55) 0 calc(50% * 2))";
  assertEquals(ctx.fillStyle, "#ff00ff");
  ctx.fillStyle = "rgba(0, 0, 255, calc(1 / 2))";
  assertEquals(ctx.fillStyle, "rgba(0, 0, 255, 0.5)");
});

Deno.test(function canvas2dFillStyleColorMix() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "color-mix(in srgb, red, blue)";
  assertEquals(ctx.fillStyle, "color(srgb 0.5 0 0.5)");
  ctx.fillStyle = "color-mix(in srgb, red 30%, blue 30%)";
  assertEquals(ctx.fillStyle, "color(srgb 0.5 0 0.5 / 0.6)");
  ctx.fillStyle = "red";
  ctx.fillStyle = "color-mix(in srgb, red 0%, blue 0%)";
  assertEquals(ctx.fillStyle, "#ff0000");
});

Deno.test(function canvas2dFillStyleRelativeColor() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "rgb(from red g r b)";
  assertEquals(ctx.fillStyle, "color(srgb 0 1 0)");
  ctx.fillStyle = "rgb(from red calc(r / 2) g b)";
  assertEquals(ctx.fillStyle, "color(srgb 0.5 0 0)");
  ctx.fillStyle = "hsl(from red calc(h + 120) s l)";
  assertEquals(ctx.fillStyle, "color(srgb 0 1 0)");
  ctx.fillStyle =
    "color(from color(srgb 0.25 0.5 0.75 / 0.5) srgb r g b / alpha)";
  assertEquals(ctx.fillStyle, "color(srgb 0.25 0.5 0.75 / 0.5)");
});

Deno.test(function canvas2dFillStyleKeywordColors() {
  const canvas = new OffscreenCanvas(10, 10);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "grey";
  assertEquals(ctx.fillStyle, "#808080");
  // OffscreenCanvas has no style context; currentcolor computes to black.
  ctx.fillStyle = "red";
  ctx.fillStyle = "currentcolor";
  assertEquals(ctx.fillStyle, "#000000");
  ctx.fillStyle = "CanvasText";
  assertEquals(ctx.fillStyle, "#000000");
  ctx.fillStyle = "ThreeDDarkShadow";
  assertEquals(ctx.fillStyle, "#767676");
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
  permissions: { read: true, env: true, sys: ["localFonts"] },
  ignore: isWsl || isCIWithoutGPU || !hasCanvasRenderer,
}, async function canvas2dFillTextRendersGlyphs() {
  // Generic families only resolve once the system fonts are registered.
  await Deno.registerLocalFonts();
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

// Metrics of tests/testdata/NotoSansCJKjp-Regular-subset-halt-min.otf, which
// the font-relative length tests below resolve against. unitsPerEm is 1000, so
// each ratio is the raw font unit divided by 1000.
const TEST_FONT_PATH = "../testdata/NotoSansCJKjp-Regular-subset-halt-min.otf";
/** OS/2 sxHeight (543). */
const TEST_FONT_EX = 0.543;
/** OS/2 sCapHeight (733). */
const TEST_FONT_CAP = 0.733;
/** Advance of `0` (555). */
const TEST_FONT_CH = 0.555;
/** hhea ascender - descender + lineGap (1160 + 288 + 0). */
const TEST_FONT_LH = 1.448;
/** Spacing is applied at f32 precision, so exact equality is too strict. */
const SPACING_EPSILON = 1e-3;

async function withTestFont<T>(
  family: string,
  fn: (ctx: OffscreenCanvasRenderingContext2D) => T,
): Promise<T> {
  const bytes = await Deno.readFile(new URL(TEST_FONT_PATH, import.meta.url));
  const face = new FontFace(family, bytes);
  Deno.fonts.add(face);
  try {
    await Deno.fonts.ready;
    return fn(new OffscreenCanvas(400, 200).getContext("2d")!);
  } finally {
    Deno.fonts.delete(face);
  }
}

Deno.test(
  { permissions: { read: true } },
  async function canvas2dSpacingFontRelativeUnits() {
    await withTestFont("SpacingUnitsFont", (ctx) => {
      ctx.font = "100px SpacingUnitsFont";
      const base = ctx.measureText("AA").width;
      // letterSpacing is added after every character, so "AA" grows by twice
      // the resolved spacing.
      const perChar = (spacing: string) => {
        ctx.letterSpacing = spacing;
        const width = ctx.measureText("AA").width;
        ctx.letterSpacing = "0px";
        return (width - base) / 2;
      };

      // https://www.w3.org/TR/css-values-4/#font-relative-lengths
      assertAlmostEquals(perChar("1em"), 100, SPACING_EPSILON);
      assertAlmostEquals(perChar("1ex"), 100 * TEST_FONT_EX, SPACING_EPSILON);
      assertAlmostEquals(perChar("1cap"), 100 * TEST_FONT_CAP, SPACING_EPSILON);
      assertAlmostEquals(perChar("1ch"), 100 * TEST_FONT_CH, SPACING_EPSILON);
      assertAlmostEquals(perChar("1lh"), 100 * TEST_FONT_LH, SPACING_EPSILON);

      // Canvas has no root element, so the root units see the same font.
      assertAlmostEquals(perChar("1rem"), 100, SPACING_EPSILON);
      assertAlmostEquals(perChar("1rex"), 100 * TEST_FONT_EX, SPACING_EPSILON);
      assertAlmostEquals(
        perChar("1rcap"),
        100 * TEST_FONT_CAP,
        SPACING_EPSILON,
      );
      assertAlmostEquals(perChar("1rch"), 100 * TEST_FONT_CH, SPACING_EPSILON);
      assertAlmostEquals(perChar("1rlh"), 100 * TEST_FONT_LH, SPACING_EPSILON);
    });
  },
);

Deno.test(
  { permissions: { read: true } },
  async function canvas2dSpacingFontRelativeUnitsTrackTheFont() {
    await withTestFont("SpacingTrackFont", (ctx) => {
      ctx.font = "20px SpacingTrackFont";
      ctx.letterSpacing = "1ex";
      const narrow = ctx.measureText("AA").width;
      ctx.font = "40px SpacingTrackFont";
      // The specified value is retained, so it re-resolves against the new
      // font instead of staying at the pixel value it first computed to.
      assertEquals(ctx.letterSpacing, "1ex");
      assertAlmostEquals(
        ctx.measureText("AA").width,
        narrow * 2,
        SPACING_EPSILON,
      );
    });
  },
);

Deno.test(function canvas2dSpacingUnitRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  for (
    const unit of [
      "em",
      "cap",
      "ch",
      "ex",
      "ic",
      "lh",
      "rem",
      "rcap",
      "rch",
      "rex",
      "ric",
      "rlh",
      "vw",
      "svh",
      "lvi",
      "dvb",
      "vmin",
      "dvmax",
      "cqw",
      "cqmin",
    ]
  ) {
    ctx.letterSpacing = `1${unit.toUpperCase()}`;
    assertEquals(ctx.letterSpacing, `1${unit}`);
    ctx.wordSpacing = `2${unit}`;
    assertEquals(ctx.wordSpacing, `2${unit}`);
  }
  ctx.letterSpacing = "0px";
  ctx.wordSpacing = "0px";
});

Deno.test(function canvas2dViewportUnitsResolveToZero() {
  // Canvas has no viewport and no query container, so the initial containing
  // block is zero-sized. The value still round-trips, per the setter steps.
  // https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-letterspacing
  // https://drafts.csswg.org/css-conditional-5/#container-lengths
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const base = ctx.measureText("AA").width;
  for (const value of ["10vw", "10dvh", "10cqmin"]) {
    ctx.letterSpacing = value;
    assertEquals(ctx.letterSpacing, value);
    assertEquals(ctx.measureText("AA").width, base);
  }
  ctx.letterSpacing = "0px";

  ctx.font = "10vw sans-serif";
  assertEquals(ctx.font, "0px sans-serif");
});

Deno.test(function canvas2dFontShorthandRelativeUnits() {
  // The shorthand resolves against the parent font, which for a canvas with no
  // element is the `10px sans-serif` default -- never against the font being
  // set. Only `em` and `rem` are asserted here because the metric-based units
  // depend on whichever face backs `sans-serif`; their fallbacks are covered by
  // `relative_size_resolves_against_default_10px` in ext/web/css/font.rs.
  // https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.font = "1em sans-serif";
  assertEquals(ctx.font, "10px sans-serif");
  // No root element, so `rem` sees the same default font rather than 16px.
  ctx.font = "1rem sans-serif";
  assertEquals(ctx.font, "10px sans-serif");
  ctx.font = "200% sans-serif";
  assertEquals(ctx.font, "20px sans-serif");
});

Deno.test(function canvas2dFontShorthandLengthPercentage() {
  // font-size takes a <length-percentage>, so percentages mix with lengths.
  // https://drafts.csswg.org/css-fonts-4/#font-size-prop
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  for (
    const [value, expected] of [
      ["calc(0.5em + 50%) sans-serif", "10px sans-serif"],
      ["calc(50% - 0.2em) sans-serif", "3px sans-serif"],
      ["min(0.5em, 50%) sans-serif", "5px sans-serif"],
      ["clamp(50%, 1em, 200%) sans-serif", "10px sans-serif"],
    ]
  ) {
    ctx.font = value;
    assertEquals(ctx.font, expected, value);
  }

  // A percentage counts as a length, so these do not type-check.
  ctx.font = "10px sans-serif";
  for (const value of ["calc(1px * 50%) sans-serif", "calc(50% + 1) serif"]) {
    ctx.font = value;
    assertEquals(ctx.font, "10px sans-serif", value);
  }
});

Deno.test(function canvas2dSpacingRejectsPercentages() {
  // The spacing attributes take a plain <length>, which leaves a percentage
  // nothing to be 1% of.
  // https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-letterspacing
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.letterSpacing = "2px";
  for (const value of ["50%", "calc(1em + 50%)", "min(1em, 50%)"]) {
    ctx.letterSpacing = value;
    assertEquals(ctx.letterSpacing, "2px", value);
  }
  ctx.letterSpacing = "0px";
});

Deno.test(
  { permissions: { read: true } },
  async function canvas2dSpacingMathFunctionsTrackTheFont() {
    await withTestFont("SpacingCalcFont", (ctx) => {
      ctx.font = "10px SpacingCalcFont";
      const base = (spacing: string) => {
        ctx.letterSpacing = spacing;
        const width = ctx.measureText("AA").width;
        ctx.letterSpacing = "0px";
        return (width - ctx.measureText("AA").width) / 2;
      };

      // A math function over font-relative units is kept as a tree, so it
      // re-resolves against whichever font is in effect.
      assertEquals(ctx.letterSpacing, "0px");
      assertAlmostEquals(base("calc(1em + 2px)"), 12, SPACING_EPSILON);
      assertAlmostEquals(base("min(1em, 15px)"), 10, SPACING_EPSILON);
      assertAlmostEquals(base("clamp(5px, 1em, 15px)"), 10, SPACING_EPSILON);
      ctx.font = "100px SpacingCalcFont";
      assertAlmostEquals(base("calc(1em + 2px)"), 102, SPACING_EPSILON);
      assertAlmostEquals(base("min(1em, 15px)"), 15, SPACING_EPSILON);
      assertAlmostEquals(base("clamp(5px, 1em, 15px)"), 15, SPACING_EPSILON);
      assertAlmostEquals(base("hypot(3em, 4em)"), 500, SPACING_EPSILON);

      // The font dependency of `sqrt(1em / 1px)` flows through a <number>, and
      // of `atan2(1em, 1px)` through an <angle>. The tree holds both, because
      // its nodes carry no dimension of their own.
      assertAlmostEquals(
        base("calc(sqrt(1em / 1px) * 1px)"),
        10,
        SPACING_EPSILON,
      );
      ctx.font = "25px SpacingCalcFont";
      assertAlmostEquals(
        base("calc(sqrt(1em / 1px) * 1px)"),
        5,
        SPACING_EPSILON,
      );
      assertAlmostEquals(
        base("calc(atan2(1em, 1px) / 1deg * 1px)"),
        Math.atan2(25, 1) * 180 / Math.PI,
        SPACING_EPSILON,
      );
      ctx.font = "100px SpacingCalcFont";
      assertAlmostEquals(
        base("calc(atan2(1em, 1px) / 1deg * 1px)"),
        Math.atan2(100, 1) * 180 / Math.PI,
        SPACING_EPSILON,
      );
    });
  },
);

Deno.test(function canvas2dSpacingMathFunctionSerialization() {
  // https://www.w3.org/TR/css-values-4/#calc-serialize
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  for (
    const [value, expected] of [
      // Sum terms are sorted by unit, ASCII case-insensitively.
      ["calc(2px + 1em)", "calc(1em + 2px)"],
      ["calc(1em - 2px)", "calc(1em - 2px)"],
      // A tree that simplified to a single dimension drops the wrapper.
      ["calc(1em * 2)", "2em"],
      ["abs(-1em)", "1em"],
      ["min(1em, 15px)", "min(1em, 15px)"],
      ["clamp(none, 1em, 15px)", "clamp(none, 1em, 15px)"],
      ["round(to-zero, 1em, 3px)", "round(to-zero, 1em, 3px)"],
      ["calc(min(1em, 15px) + 1px)", "calc(1px + min(1em, 15px))"],
      // Viewport units resolve to zero, but are still retained symbolically.
      ["calc(1vw + 2px)", "calc(2px + 1vw)"],
      ["min(1cqmin, 15px)", "min(1cqmin, 15px)"],
      // An absolute-only expression is already exact, so it collapses.
      ["calc(1px + 2px)", "3px"],
      // A dependency that leaves the length dimension keeps its nodes, and a
      // product serializes in its authored order.
      ["calc(sqrt(1em / 1px) * 1px)", "calc(sqrt(1em / 1px) * 1px)"],
      ["calc(1em / 1px * 1px)", "calc(1em / 1px * 1px)"],
      ["calc(pow(1em / 1px, 2) * 1px)", "calc(pow(1em / 1px, 2) * 1px)"],
      ["calc(sign(1em) * 2px)", "calc(sign(1em) * 2px)"],
      [
        "calc(atan2(1em, 1px) / 1deg * 1px)",
        "calc(atan2(1em, 1px) / 1deg * 1px)",
      ],
    ]
  ) {
    ctx.letterSpacing = value;
    assertEquals(ctx.letterSpacing, expected, `serializing ${value}`);
  }
  ctx.letterSpacing = "0px";
});

Deno.test(function canvas2dFilterFontRelativeLengths() {
  // Relative lengths in a <filter-value-list> resolve against the default value
  // of the `font` attribute, so they are accepted rather than rejected. The
  // getter returns the specified string.
  // https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-filter
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  for (
    const value of [
      "blur(1em)",
      "blur(2rem)",
      "blur(1ex)",
      "blur(1lh)",
      "blur(10vw)",
      "drop-shadow(1em 2ex 1em red)",
    ]
  ) {
    ctx.filter = value;
    assertEquals(ctx.filter, value);
    ctx.filter = "none";
  }

  // A negative blur is still invalid, so the previous value is kept.
  ctx.filter = "blur(2px)";
  ctx.filter = "blur(-1em)";
  assertEquals(ctx.filter, "blur(2px)");
});

Deno.test(function canvas2dLangDefault() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.lang, "inherit");
  // Deno has no document, so an unparsable tag is kept verbatim and simply
  // resolves to no locale.
  ctx.lang = "not-a-real-lang";
  assertEquals(ctx.lang, "not-a-real-lang");
});

// The test font carries a `locl` feature with JAN / KOR / ZHS / ZHT / ZHH
// LangSys entries. Korean substitutes a wider U+0020 (280 vs 224 per em),
// which Japanese leaves alone, so `lang` is observable through measureText.
Deno.test(
  { permissions: { read: true } },
  async function canvas2dLangSelectsLanguageSpecificGlyphs() {
    await withTestFont("LangFont", (ctx) => {
      ctx.font = "100px LangFont";
      const widthFor = (lang: string) => {
        ctx.lang = lang;
        return ctx.measureText("A A").width;
      };

      const ja = widthFor("ja");
      assertAlmostEquals(widthFor("ko"), ja + 100 * (0.280 - 0.224));
      // No locale means no `locl`, which for this font matches Japanese.
      assertEquals(widthFor("inherit"), ja);
      assertEquals(widthFor(""), ja);
      assertEquals(widthFor("en"), ja);
    });
  },
);

Deno.test(
  { permissions: { read: true }, ignore: !hasCanvasRenderer },
  async function canvas2dLangAppliesToFillText() {
    const inkWidth = (ctx: OffscreenCanvasRenderingContext2D, lang: string) => {
      ctx.clearRect(0, 0, 400, 200);
      ctx.lang = lang;
      ctx.fillStyle = "black";
      ctx.fillText("A A", 10, 120);
      const { data } = ctx.getImageData(0, 0, 400, 200);
      let min = Infinity;
      let max = -Infinity;
      for (let y = 0; y < 200; y++) {
        for (let x = 0; x < 400; x++) {
          if (data[(y * 400 + x) * 4 + 3] !== 0) {
            min = Math.min(min, x);
            max = Math.max(max, x);
          }
        }
      }
      return max - min;
    };

    await withTestFont("LangDrawFont", (ctx) => {
      ctx.font = "100px LangDrawFont";
      // Drawing goes through the same layout as measureText, so the wider
      // Korean space shows up in the rendered ink too.
      assert(inkWidth(ctx, "ko") > inkWidth(ctx, "ja"));
    });
  },
);

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

Deno.test(async function fontFaceConstructorRejectsInvalidSource() {
  // An unparseable `src` descriptor errors the face rather than throwing.
  const face = new FontFace("TestFont", "not-a-src-descriptor");
  assertEquals(face.status, "error");
  const error = await assertRejects(
    () => face.load(),
    DOMException,
    "Could not parse the source as a CSS src descriptor",
  );
  assertEquals(error.name, "SyntaxError");
  // A source of the wrong type is a binding error, which does throw.
  // @ts-expect-error: testing wrong source type
  assertThrows(() => new FontFace("TestFont", 42), TypeError);
});

Deno.test(function fontFaceConstructorAcceptsSrcDescriptor() {
  const face = new FontFace("TestFont", 'url("blob:null/abc")');
  assertEquals(face.family, "TestFont");
  assertEquals(face.status, "unloaded");
  const withHints = new FontFace(
    "TestFont",
    'local("Other"), url("blob:null/abc") format("truetype")',
  );
  assertEquals(withHints.status, "unloaded");
});

Deno.test(
  { permissions: { read: true } },
  async function fontFaceLoadFromBlobUrl() {
    const data = await Deno.readFile(
      "tests/testdata/NotoSerifCJKjp-Regular-subset.otf",
    );
    const url = URL.createObjectURL(new Blob([data]));
    try {
      const face = new FontFace("BlobUrlFont", `url("${url}")`);
      await face.load();
      assertEquals(face.status, "loaded");
      // Descriptors not given by the caller come from the font file.
      assertEquals(face.weight, "200");
      assertEquals(face.style, "normal");
      assertEquals(face.stretch, "normal");
    } finally {
      URL.revokeObjectURL(url);
    }
  },
);

Deno.test(async function fontFaceLoadRejectsWoff2() {
  // WOFF2 is not an SFNT container, so `fontique` cannot read it.
  const woff2 = new Uint8Array([
    0x77,
    0x4F,
    0x46,
    0x32, // "wOF2"
    0x00,
    0x01,
    0x00,
    0x00,
  ]);
  const face = new FontFace("Woff2Font", woff2);
  await assertRejects(
    () => face.load(),
    DOMException,
    "No valid font faces in data",
  );
  assertEquals(face.status, "error");
});

Deno.test(async function fontFaceLoadRejectsRevokedBlobUrl() {
  const url = URL.createObjectURL(new Blob([new Uint8Array(4)]));
  URL.revokeObjectURL(url);
  const face = new FontFace("StaleFont", `url("${url}")`);
  await assertRejects(() => face.load(), DOMException, "no longer valid");
  assertEquals(face.status, "error");
});

Deno.test(
  { permissions: { sys: ["localFonts"] } },
  async function fontFaceLoadRejectsMissingLocalFont() {
    const face = new FontFace("LocalFont", 'local("No Such Font Installed")');
    const error = await assertRejects(
      () => face.load(),
      DOMException,
      "no usable source",
    );
    assertEquals(error.name, "NetworkError");
    assertEquals(face.status, "error");
  },
);

Deno.test(
  { permissions: { sys: [] } },
  async function fontFaceLoadLocalRequiresSysPermission() {
    const face = new FontFace("LocalFont", 'local("Arial")');
    await assertRejects(() => face.load(), Deno.errors.NotCapable);
    assertEquals(face.status, "error");
  },
);

Deno.test(
  // No net permission: reaching the fetch at all would throw NotCapable, so a
  // NetworkError proves the entry was dropped by its format()/tech().
  { permissions: { net: [] } },
  async function fontFaceSkipsUnsupportedFormatAndTech() {
    for (
      const src of [
        'url("https://example.com/f.woff2") format("woff2")',
        'url("https://example.com/f.woff") format("woff")',
        'url("https://example.com/f.svg") format("svg")',
        'url("https://example.com/f.ttf") tech(color-SVG)',
        'url("https://example.com/f.ttf") tech(features-graphite)',
      ]
    ) {
      const face = new FontFace("SkipFont", src);
      await assertRejects(
        () => face.load(),
        DOMException,
        "no usable source",
        `${src} should be skipped without fetching`,
      );
    }
  },
);

Deno.test(
  { permissions: { net: [] } },
  async function fontFaceKeepsSupportedFormatAndTech() {
    for (
      const src of [
        'url("https://example.com/f.ttf") format("truetype")',
        'url("https://example.com/f.otf") format("opentype")',
        'url("https://example.com/f.ttc") format("collection")',
        'url("https://example.com/f.ttf") tech(variations, color-COLRv1)',
        'url("https://example.com/f.ttf") tech(color-sbix, palettes)',
      ]
    ) {
      const face = new FontFace("KeepFont", src);
      // Reaching the permission check means the entry was not filtered out.
      await assertRejects(
        () => face.load(),
        Deno.errors.NotCapable,
        undefined,
        `${src} should be fetched`,
      );
    }
  },
);

Deno.test(
  { permissions: { net: [] } },
  async function fontFaceLoadUrlRequiresNetPermission() {
    const face = new FontFace("RemoteFont", 'url("https://example.com/f.otf")');
    await assertRejects(() => face.load(), Deno.errors.NotCapable);
    assertEquals(face.status, "error");
  },
);

Deno.test(
  { permissions: { read: true } },
  async function fontFaceLoadFromFileUrl() {
    const url = new URL(
      "../testdata/NotoSerifCJKjp-Regular-subset.otf",
      import.meta.url,
    );
    const face = new FontFace("FileUrlFont", `url("${url.href}")`);
    await face.load();
    assertEquals(face.status, "loaded");
  },
);

// The http body is drained by op_fontdb_load_resource, straight from the
// response resource, so it never becomes a JS ArrayBuffer.
Deno.test(
  { permissions: { net: ["localhost:4545"] } },
  async function fontFaceLoadFromHttpUrl() {
    const face = new FontFace(
      "HttpFont",
      'url("http://localhost:4545/NotoSerifCJKjp-Regular-subset.otf")',
    );
    await face.load();
    assertEquals(face.status, "loaded");
    // Descriptors not given by the caller come from the font file.
    assertEquals(face.weight, "200");
  },
);

Deno.test(
  { permissions: { read: false } },
  async function fontFaceLoadFileUrlRequiresReadPermission() {
    const face = new FontFace("FileUrlFont", 'url("file:///nope.otf")');
    await assertRejects(() => face.load(), Deno.errors.NotCapable);
  },
);

Deno.test(function fontFaceConstructorAcceptsArrayBuffer() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  assertEquals(face.family, "TestFont");
  assertEquals(face.status, "unloaded");
});

Deno.test(function fontFaceConstructorAcceptsTypedArray() {
  const face = new FontFace("TestFont", new Uint8Array([0, 1, 2, 3]));
  assertEquals(face.family, "TestFont");
});

// css-font-loading reserves SyntaxError for the BufferSource form; a url()
// source that cannot be fetched or parsed is a NetworkError, never a raw
// TypeError from fetch.
Deno.test(
  { permissions: {} },
  async function fontFaceLoadUrlRejectsWithNetworkError() {
    for (
      const [label, src] of [
        // Unparsable whatever the base is; `new Request` throws a TypeError.
        // The relative-url-without---location case is covered by
        // tests/specs/run/unstable_canvas2d_font_face_url.
        ["invalid url", 'url("http://[")'],
        ["unsupported scheme", 'url("wss://example.com/f.otf")'],
        // Fetched fine, but the bytes are not a font.
        ["unparsable data url", 'url("data:font/otf;base64,AAECAwQFBgc=")'],
      ]
    ) {
      const face = new FontFace("UrlFont", src);
      const error = await assertRejects(
        () => face.load(),
        DOMException,
        undefined,
        label,
      );
      assertEquals(error.name, "NetworkError", label);
      assertEquals(face.status, "error", label);
    }
  },
);

Deno.test(
  { permissions: { read: true } },
  async function fontFaceLoadUrlRejectsNonFontFile() {
    const url = new URL("./canvas2d_test.ts", import.meta.url);
    const face = new FontFace("NotAFont", `url("${url.href}")`);
    const error = await assertRejects(() => face.load(), DOMException);
    assertEquals(error.name, "NetworkError");
  },
);

// The constructor stores a copy, and op_fontdb_load detaches only that copy.
Deno.test(async function fontFaceConstructorCopiesBufferSource() {
  const bytes = new Uint8Array(
    await Deno.readFile(
      new URL("../testdata/NotoSerifCJKjp-Regular-subset.otf", import.meta.url),
    ),
  );
  const size = bytes.byteLength;
  const face = new FontFace("CopiedFont", bytes);
  // Wiping the source after construction must not affect the stored font.
  bytes.fill(0);
  await face.load();
  assertEquals(face.status, "loaded");
  // The caller's buffer survives the detach.
  assertEquals(bytes.byteLength, size);
});

Deno.test(async function fontFaceLoadDoesNotDetachCallerBuffer() {
  const buffer = new Uint8Array([0, 1, 2, 3]).buffer;
  const face = new FontFace("DetachFont", buffer);
  await assertRejects(() => face.load(), DOMException);
  assertEquals(buffer.byteLength, 4);
});

Deno.test(function fontFaceDefaultDescriptors() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  assertEquals(face.style, "normal");
  assertEquals(face.weight, "normal");
  assertEquals(face.width, "normal");
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

// CSS Font Loading: the constructor does not throw for a descriptor it cannot
// parse. The face lands in "error" and load() rejects with a SyntaxError.
Deno.test(async function fontFaceConstructorErrorsOnInvalidDescriptors() {
  for (
    const descriptors of [
      { style: "invalid-style" },
      { weight: "0" },
      { stretch: "invalid-stretch" },
      { width: "-1%" },
      { unicodeRange: "nonsense" },
      // `display` is a typed enum, so an invalid value needs a cast to reach
      // the runtime validation this test is about.
      { display: "invalid-display" as FontDisplay },
      { ascentOverride: "-50%" },
      { descentOverride: "10px" },
      { lineGapOverride: "nope" },
      { featureSettings: "bogus" },
      { variationSettings: "bogus" },
    ]
  ) {
    const label = JSON.stringify(descriptors);
    const face = new FontFace("TestFont", new ArrayBuffer(4), descriptors);
    assertEquals(face.status, "error", label);
    const error = await assertRejects(
      () => face.load(),
      DOMException,
      undefined,
      label,
    );
    assertEquals(error.name, "SyntaxError", label);
  }
});

// --- FontFace property setters ---

Deno.test(function fontFaceFamilySetter() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.family = "NewName";
  assertEquals(face.family, "NewName");
});

// Invalid / generic family names are quoted rather than rejected
// (https://github.com/w3c/csswg-drafts/issues/6236).
Deno.test(function fontFaceFamilyQuotesInvalidNames() {
  for (
    const raw of [
      "content:Segoe UI",
      "sans-serif",
      "A, B",
      "inherit",
      "a 1",
      "",
      "a  b",
      " a b",
      "a b ",
    ]
  ) {
    const face = new FontFace(raw, new ArrayBuffer(4));
    assertEquals(
      face.family,
      `"${raw}"`,
      `constructor: ${JSON.stringify(raw)}`,
    );
    assert(face.status !== "error", `constructor: ${JSON.stringify(raw)}`);
    face.family = "ValidFont";
    face.family = raw;
    assertEquals(face.family, `"${raw}"`, `setter: ${JSON.stringify(raw)}`);
  }
  // Valid names stay unquoted.
  const valid = new FontFace("Times New Roman", new ArrayBuffer(4));
  assertEquals(valid.family, "Times New Roman");
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

Deno.test(function fontFaceWidthAliasesStretch() {
  const face = new FontFace("TestFont", new ArrayBuffer(4), {
    width: "200%",
  });
  assertEquals(face.width, "200%");
  assertEquals(face.stretch, "200%");

  face.stretch = "50%";
  assertEquals(face.width, "50%");
  face.width = "87.5%";
  assertEquals(face.stretch, "87.5%");
});

Deno.test(function fontFaceWidthDescriptorTakesPriorityOverStretch() {
  const face = new FontFace("TestFont", new ArrayBuffer(4), {
    stretch: "50%",
    width: "200%",
  });
  assertEquals(face.width, "200%");
  assertEquals(face.stretch, "200%");
});

Deno.test(function fontFaceWidthSetterThrowsOnInvalid() {
  const face = new FontFace("TestFont", new ArrayBuffer(4));
  face.width = "expanded";
  assertThrows(() => {
    face.width = "-1%";
  }, DOMException);
  assertEquals(face.width, "expanded");
  assertEquals(face.stretch, "expanded");
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

// Width keywords and percentages are equivalent under CSS font matching.
Deno.test(
  { permissions: { read: true } },
  async function fontFaceSetCheckMatchesWidthPercentageWithKeyword() {
    const bytes = await Deno.readFile(
      new URL(
        "../testdata/NotoSansCJKjp-Regular-subset-halt-min.otf",
        import.meta.url,
      ),
    );
    const face = new FontFace("WidthMatchFont", bytes, { width: "87.5%" });
    Deno.fonts.add(face);
    try {
      await face.load();
      // Face width 87.5% ≡ query keyword semi-condensed.
      assert(Deno.fonts.check("semi-condensed 12px WidthMatchFont"));
      // Only one face in the family → nearest match for any width query.
      assert(Deno.fonts.check("condensed 12px WidthMatchFont"));
      assert(Deno.fonts.check("normal 12px WidthMatchFont"));

      const loaded = await Deno.fonts.load(
        "semi-condensed 12px WidthMatchFont",
      );
      assertEquals(loaded.length, 1);
      assertEquals(loaded[0], face);
    } finally {
      Deno.fonts.delete(face);
    }
  },
);

// Among several widths, CSS nearest-width rules pick the right face.
Deno.test(
  { permissions: { read: true } },
  async function fontFaceSetCheckNearestWidthAmongFaces() {
    const bytes = await Deno.readFile(
      new URL(
        "../testdata/NotoSansCJKjp-Regular-subset-halt-min.otf",
        import.meta.url,
      ),
    );
    const condensed = new FontFace("NearestWidthFont", bytes, {
      width: "75%",
    });
    const normal = new FontFace("NearestWidthFont", bytes.slice(), {
      width: "100%",
    });
    Deno.fonts.add(condensed);
    Deno.fonts.add(normal);
    try {
      await condensed.load();
      await normal.load();
      // Desired 87.5% ≤ 100%: prefer the face below (75%) over above (100%).
      const loaded = await Deno.fonts.load(
        "semi-condensed 12px NearestWidthFont",
      );
      assertEquals(loaded.length, 1);
      assertEquals(loaded[0].width, "75%");
      assert(Deno.fonts.check("semi-condensed 12px NearestWidthFont"));
    } finally {
      Deno.fonts.delete(condensed);
      Deno.fonts.delete(normal);
    }
  },
);

// bold maps to the nearest available weight (regular) when no 700 face exists.
Deno.test(
  { permissions: { read: true } },
  async function fontFaceSetCheckNearestWeight() {
    const bytes = await Deno.readFile(
      new URL(
        "../testdata/NotoSansCJKjp-Regular-subset-halt-min.otf",
        import.meta.url,
      ),
    );
    const face = new FontFace("NearestWeightFont", bytes, { weight: "400" });
    Deno.fonts.add(face);
    try {
      await face.load();
      assert(Deno.fonts.check("bold 12px NearestWeightFont"));
      const loaded = await Deno.fonts.load("bold 12px NearestWeightFont");
      assertEquals(loaded.length, 1);
      assertEquals(loaded[0], face);
    } finally {
      Deno.fonts.delete(face);
    }
  },
);

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
      // Only a regular face exists; CSS nearest-weight still selects it for bold.
      assert(Deno.fonts.check("bold 12px NotoSansCJK", "日"));
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

// Whether the system fonts are visible is process-wide state, so the gate
// itself is covered by tests/specs/run/unstable_canvas2d_system_fonts.

Deno.test(
  { permissions: { read: true, sys: [] } },
  async function fontFaceWorksWithoutLoadLocalFonts() {
    // A registered FontFace is the caller's own data, so it needs no
    // permission even though the system fonts stay hidden.
    const bytes = await Deno.readFile(
      new URL(
        "../testdata/NotoSansCJKjp-Regular-subset-halt-min.otf",
        import.meta.url,
      ),
    );
    const face = new FontFace("UngatedFont", bytes);
    Deno.fonts.add(face);
    try {
      await Deno.fonts.ready;
      const ctx = new OffscreenCanvas(100, 30).getContext("2d")!;
      ctx.font = "20px UngatedFont";
      assert(ctx.measureText("国").width > 0);
    } finally {
      Deno.fonts.delete(face);
    }
  },
);

Deno.test(
  { permissions: { read: true } },
  async function fontFaceDescriptorsFromSetterAreUsedForMatching() {
    const bytes = await Deno.readFile(
      new URL(
        "../testdata/NotoSansCJKjp-Regular-subset-halt-min.otf",
        import.meta.url,
      ),
    );
    const bold = new FontFace("SetterFont", bytes, { weight: "700" });
    // A second face at 400 so nearest-weight can prefer it for `normal`.
    const regular = new FontFace("SetterFont", bytes.slice(), {
      weight: "400",
    });
    // Assigning after construction counts as user-specified, so loading must
    // not overwrite it with the font file's own weight.
    bold.weight = "700";
    Deno.fonts.add(bold);
    Deno.fonts.add(regular);
    try {
      await Deno.fonts.ready;
      assertEquals(bold.weight, "700");
      assertEquals(regular.weight, "400");
      const boldFaces = await Deno.fonts.load("bold 12px SetterFont");
      assertEquals(boldFaces.length, 1);
      assertEquals(boldFaces[0], bold);
      const normalFaces = await Deno.fonts.load("12px SetterFont");
      assertEquals(normalFaces.length, 1);
      assertEquals(normalFaces[0], regular);
    } finally {
      Deno.fonts.delete(bold);
      Deno.fonts.delete(regular);
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
  { permissions: { sys: ["localFonts"] } },
  async function registerLocalFontsSucceeds() {
    await Deno.registerLocalFonts();
  },
);

Deno.test(
  { permissions: { sys: [] } },
  async function registerLocalFontsRequiresPermission() {
    await assertRejects(
      () => Deno.registerLocalFonts(),
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
  // @ts-expect-error: invalid fillRule value
  assertThrows(() => ctx.isPointInPath(50, 50, "gazonk"), TypeError);
  const path = new Path2D();
  path.rect(0, 0, 100, 100);
  // @ts-expect-error: invalid fillRule value
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

// --- Gradients and patterns ---

Deno.test(function canvas2dCanvasGradientExists() {
  assert(globalThis.CanvasGradient !== undefined);
  assertEquals(
    typeof globalThis.CanvasGradient.prototype.addColorStop,
    "function",
  );
});

Deno.test(function canvas2dCanvasPatternExists() {
  assert(globalThis.CanvasPattern !== undefined);
  assertEquals(
    typeof globalThis.CanvasPattern.prototype.setTransform,
    "function",
  );
});

Deno.test(function canvas2dCanvasGradientIllegalConstructor() {
  // @ts-ignore: testing illegal constructor
  assertThrows(() => new CanvasGradient(), TypeError);
});

Deno.test(function canvas2dCanvasPatternIllegalConstructor() {
  // @ts-ignore: testing illegal constructor
  assertThrows(() => new CanvasPattern(), TypeError);
});

Deno.test(function canvas2dCreateLinearGradient() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, 10, 10);
  assert(g instanceof globalThis.CanvasGradient);
});

Deno.test(function canvas2dCreateRadialGradient() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createRadialGradient(0, 0, 1, 5, 5, 5);
  assert(g instanceof globalThis.CanvasGradient);
});

Deno.test(function canvas2dCreateConicGradient() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createConicGradient(0, 5, 5);
  assert(g instanceof globalThis.CanvasGradient);
});

Deno.test({
  ignore: !hasCanvasRenderer,
}, function canvas2dCreateConicGradientNormalizesStartAngle() {
  function sample(startAngle: number): number[][] {
    const canvas = new OffscreenCanvas(100, 50);
    const ctx = canvas.getContext("2d")!;
    const gradient = ctx.createConicGradient(startAngle, 50, 25);
    gradient.addColorStop(0, "#f00");
    gradient.addColorStop(0.25, "#0f0");
    gradient.addColorStop(0.5, "#0f0");
    gradient.addColorStop(0.75, "#f00");
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, 100, 50);
    return [
      Array.from(ctx.getImageData(25, 15, 1, 1).data),
      Array.from(ctx.getImageData(75, 40, 1, 1).data),
    ];
  }

  assertEquals(sample(3 * Math.PI / 2), sample(-Math.PI / 2));
  assertEquals(sample(2 * Math.PI), sample(0));
});

Deno.test(function canvas2dCreateLinearGradientNonFiniteThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(
    () => ctx.createLinearGradient(0, 0, NaN, 10),
    TypeError,
  );
});

Deno.test(function canvas2dAddColorStopValid() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, 10, 0);
  g.addColorStop(0, "#f00");
  g.addColorStop(1, "blue");
  g.addColorStop(0.5, "color-mix(in srgb, red, blue)");
  g.addColorStop(0.5, "rgb(from red g r b)");
});

Deno.test(function canvas2dShadowColorRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertEquals(ctx.shadowColor, "rgba(0, 0, 0, 0)");
  ctx.shadowColor = "rgba(255, 255, 255, 0.5)";
  assertEquals(ctx.shadowColor, "rgba(255, 255, 255, 0.5)");
  ctx.shadowColor = "lab(50 0 0)";
  assertEquals(ctx.shadowColor, "lab(50 0 0)");
  ctx.shadowColor = "not-a-color";
  assertEquals(ctx.shadowColor, "lab(50 0 0)");
});

Deno.test(function canvas2dAddColorStopInvalidOffsetThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, 10, 0);
  assertThrows(() => g.addColorStop(-1, "#000"), DOMException);
  assertThrows(() => g.addColorStop(2, "#000"), DOMException);
});

Deno.test(function canvas2dAddColorStopNonFiniteThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, 10, 0);
  assertThrows(() => g.addColorStop(NaN, "#000"), TypeError);
  assertThrows(() => g.addColorStop(Infinity, "#000"), TypeError);
});

Deno.test(function canvas2dAddColorStopInvalidColorThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, 10, 0);
  assertThrows(() => g.addColorStop(0.5, "not-a-color"), DOMException);
});

Deno.test(function canvas2dFillStyleGradientRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, 10, 0);
  g.addColorStop(0, "#f00");
  g.addColorStop(1, "#00f");
  ctx.fillStyle = g;
  assert((ctx.fillStyle as unknown) === g);
});

Deno.test(function canvas2dStrokeStyleGradientRoundTrip() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createRadialGradient(0, 0, 1, 5, 5, 5);
  ctx.strokeStyle = g;
  assert((ctx.strokeStyle as unknown) === g);
});

Deno.test(function canvas2dFillStyleGradientSaveRestore() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, 10, 0);
  ctx.fillStyle = g;
  ctx.save();
  ctx.fillStyle = "red";
  ctx.restore();
  assert((ctx.fillStyle as unknown) === g);
});

Deno.test(function canvas2dFillStyleInvalidGradientIgnored() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, 10, 0);
  ctx.fillStyle = g;
  // @ts-expect-error: invalid fillStyle value
  ctx.fillStyle = {};
  assert((ctx.fillStyle as unknown) === g);
});

Deno.test(function canvas2dCreatePatternNullImageThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(
    // @ts-expect-error: null is not a valid CanvasImageSource
    () => ctx.createPattern(null, "repeat"),
    TypeError,
  );
});

Deno.test(function canvas2dCreatePatternUndefinedRepetitionThrows() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("2d")!;
  assertThrows(
    // @ts-expect-error: undefined repetition throws SyntaxError
    () => ctx.createPattern(canvas, undefined),
    DOMException,
  );
});

Deno.test(function canvas2dCreatePatternNullRepetition() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#f00";
  ctx.fillRect(0, 0, 2, 2);
  const pattern = ctx.createPattern(canvas, null);
  assert(pattern instanceof globalThis.CanvasPattern);
});

Deno.test(function canvas2dCreatePatternEmptyRepetition() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#0f0";
  ctx.fillRect(0, 0, 2, 2);
  const pattern = ctx.createPattern(canvas, "");
  assert(pattern instanceof globalThis.CanvasPattern);
});

Deno.test(function canvas2dCreatePatternInvalidRepetitionThrows() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("2d")!;
  assertThrows(
    () => ctx.createPattern(canvas, "invalid"),
    DOMException,
  );
});

Deno.test(function canvas2dFillStylePatternRoundTrip() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#00f";
  ctx.fillRect(0, 0, 2, 2);
  const pattern = ctx.createPattern(canvas, "repeat")!;
  ctx.fillStyle = pattern;
  assert((ctx.fillStyle as unknown) === pattern);
});

Deno.test(function canvas2dPatternSetTransform() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#f00";
  ctx.fillRect(0, 0, 2, 2);
  const pattern = ctx.createPattern(canvas, "repeat")!;
  pattern.setTransform({ a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 });
});

Deno.test(
  { permissions: { sys: ["localFonts"] } },
  async function queryLocalFontsReturnsArray() {
    const fonts = await queryLocalFonts();
    assert(Array.isArray(fonts));
    assert(fonts.length > 0);
    const font = fonts[0];
    assertStrictEquals(typeof font.postscriptName, "string");
    assertStrictEquals(typeof font.fullName, "string");
    assertStrictEquals(typeof font.family, "string");
    assertStrictEquals(typeof font.style, "string");
    assert(font instanceof FontData);
  },
);

Deno.test(
  { permissions: { sys: ["localFonts"] } },
  async function queryLocalFontsSorted() {
    const fonts = await queryLocalFonts();
    for (let i = 1; i < fonts.length; i++) {
      assert(fonts[i].postscriptName >= fonts[i - 1].postscriptName);
    }
  },
);

Deno.test(
  { permissions: { sys: ["localFonts"] } },
  async function queryLocalFontsEmptyFilter() {
    const fonts = await queryLocalFonts({ postscriptNames: [] });
    assertStrictEquals(fonts.length, 0);
  },
);

Deno.test(
  { permissions: { sys: ["localFonts"] } },
  async function queryLocalFontsFilter() {
    const allFonts = await queryLocalFonts();
    if (allFonts.length === 0) return;
    const target = allFonts[0].postscriptName;
    const filtered = await queryLocalFonts({
      postscriptNames: [target],
    });
    assertStrictEquals(filtered.length, 1);
    assertStrictEquals(filtered[0].postscriptName, target);
  },
);

Deno.test(
  { permissions: { sys: ["localFonts"] } },
  async function queryLocalFontsBlobReturnsBlob() {
    const fonts = await queryLocalFonts();
    if (fonts.length === 0) return;
    const blob = await fonts[0].blob();
    assert(blob instanceof Blob);
    assertStrictEquals(blob.type, "application/octet-stream");
    assert(blob.size > 0);
  },
);

Deno.test(
  { permissions: { sys: [] } },
  async function queryLocalFontsRequiresPermission() {
    await assertRejects(
      () => queryLocalFonts(),
      Deno.errors.NotCapable,
    );
  },
);

// === Phase 5: Image Operations ===

Deno.test(function canvas2dCreateImageDataWithDimensions() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const data = ctx.createImageData(5, 3);
  assertEquals(data.width, 5);
  assertEquals(data.height, 3);
  assertEquals(data.data.length, 5 * 3 * 4);
  assert(data.data.every((v: number) => v === 0));
});

Deno.test(function canvas2dCreateImageDataNegativeDimensions() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const data = ctx.createImageData(-4, -6);
  assertEquals(data.width, 4);
  assertEquals(data.height, 6);
});

Deno.test(function canvas2dCreateImageDataZeroThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(() => ctx.createImageData(0, 5), DOMException);
  assertThrows(() => ctx.createImageData(5, 0), DOMException);
});

Deno.test(function canvas2dCreateImageDataFromImageData() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const src = new ImageData(7, 4);
  src.data[0] = 255;
  const copy = ctx.createImageData(src);
  assertEquals(copy.width, 7);
  assertEquals(copy.height, 4);
  assertEquals(copy.data[0], 0);
});

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dPutImageDataBasic() {
    const canvas = new OffscreenCanvas(4, 4);
    const ctx = canvas.getContext("2d")!;
    const imgData = ctx.createImageData(2, 2);
    imgData.data.set([
      255,
      0,
      0,
      255,
      0,
      255,
      0,
      255,
      0,
      0,
      255,
      255,
      128,
      128,
      128,
      255,
    ]);
    ctx.putImageData(imgData, 1, 1);
    const result = ctx.getImageData(1, 1, 2, 2);
    assertEquals(result.data[0], 255);
    assertEquals(result.data[1], 0);
    assertEquals(result.data[2], 0);
    assertEquals(result.data[3], 255);
  },
);

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dPutImageDataBypassesTransform() {
    const canvas = new OffscreenCanvas(4, 4);
    const ctx = canvas.getContext("2d")!;
    ctx.translate(2, 2);
    ctx.globalAlpha = 0.5;
    const imgData = ctx.createImageData(2, 2);
    for (let i = 0; i < imgData.data.length; i += 4) {
      imgData.data[i] = 255;
      imgData.data[i + 3] = 255;
    }
    ctx.putImageData(imgData, 0, 0);
    const result = ctx.getImageData(0, 0, 2, 2);
    assertEquals(result.data[0], 255);
    assertEquals(result.data[3], 255);
  },
);

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dPutImageDataDirtyRect() {
    const canvas = new OffscreenCanvas(4, 4);
    const ctx = canvas.getContext("2d")!;
    const imgData = ctx.createImageData(2, 2);
    for (let i = 0; i < imgData.data.length; i += 4) {
      imgData.data[i] = 255;
      imgData.data[i + 3] = 255;
    }
    ctx.putImageData(imgData, 0, 0, 1, 0, 1, 2);
    const r00 = ctx.getImageData(0, 0, 1, 1);
    assertEquals(r00.data[0], 0);
    const r10 = ctx.getImageData(1, 0, 1, 1);
    assertEquals(r10.data[0], 255);
  },
);

Deno.test(function canvas2dDrawImageNullThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(
    // deno-lint-ignore no-explicit-any
    () => (ctx as any).drawImage(null, 0, 0),
    TypeError,
  );
});

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dDrawImageBasic() {
    const canvas = new OffscreenCanvas(4, 4);
    const ctx = canvas.getContext("2d")!;
    const src = new OffscreenCanvas(2, 2);
    const srcCtx = src.getContext("2d")!;
    srcCtx.fillStyle = "red";
    srcCtx.fillRect(0, 0, 2, 2);
    const bitmap = src.transferToImageBitmap();
    ctx.drawImage(bitmap, 1, 1);
    const result = ctx.getImageData(1, 1, 1, 1);
    assertEquals(result.data[0], 255);
    assertEquals(result.data[1], 0);
    assertEquals(result.data[2], 0);
    assertEquals(result.data[3], 255);
  },
);

Deno.test(function canvas2dDrawImageNonFiniteSilent() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  const src = new OffscreenCanvas(2, 2);
  src.getContext("2d");
  const bitmap = src.transferToImageBitmap();
  ctx.drawImage(bitmap, NaN, 0);
  ctx.drawImage(bitmap, 0, Infinity);
});

Deno.test(function canvasFilterGlobalIsNotExposed() {
  assertStrictEquals("CanvasFilter" in globalThis, false);
});

Deno.test(function canvas2dFilterPropertyDomString() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertStrictEquals(ctx.filter, "none");

  ctx.filter = "blur(5px)";
  assertStrictEquals(ctx.filter, "blur(5px)");

  // An invalid filter string leaves the current value in place.
  ctx.filter = "this string is not a filter";
  assertStrictEquals(ctx.filter, "blur(5px)");

  ctx.filter = "none";
  assertStrictEquals(ctx.filter, "none");
});

Deno.test(function canvas2dBeginLayerOptionsValidated() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.beginLayer();
  ctx.endLayer();
  // @ts-expect-error Deno's d.ts follows lib.dom, but runtime still accepts legacy null options.
  ctx.beginLayer(null);
  ctx.endLayer();
  // @ts-expect-error Deno's d.ts follows lib.dom, but runtime still validates legacy filter options.
  ctx.beginLayer({ filter: { name: "unknownFilter" } });
  ctx.endLayer();
  // @ts-expect-error Deno's d.ts follows lib.dom, but runtime still validates legacy filter options.
  ctx.beginLayer({ filter: "invalid filter strings are tolerated" });
  ctx.endLayer();

  // @ts-expect-error Deno's d.ts follows lib.dom, but runtime still rejects legacy invalid options.
  assertThrows(() => ctx.beginLayer(""), TypeError);
  assertThrows(
    // @ts-expect-error Deno's d.ts follows lib.dom, but runtime still validates legacy filter options.
    () => ctx.beginLayer({ filter: { name: "gaussianBlur" } }),
    TypeError,
  );
});

Deno.test(function canvas2dGetImageDataTooLargeThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(() => ctx.getImageData(0, 0, 2147483647, 10), TypeError);
  assertThrows(() => ctx.createImageData(2147483647, 10), TypeError);
});

// --- GPU->CPU readback fallback heuristic ---
//
// Repeated getImageData / putImageData / convertToBlob calls switch a
// GPU-backed context to the CPU backend (Chromium-style heuristic). These
// assertions are backend-agnostic: they pass on CPU-only CI and also
// exercise the actual switch on GPU machines.

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dGetImageDataAcrossReadbackThreshold() {
    const canvas = new OffscreenCanvas(200, 200);
    const ctx = canvas.getContext("2d")!;

    ctx.fillStyle = "rgb(255, 0, 0)";
    ctx.fillRect(0, 0, 200, 200);
    for (let i = 0; i < 5; i++) {
      const result = ctx.getImageData(10, 10, 1, 1);
      assertEquals(result.data[0], 255);
      assertEquals(result.data[1], 0);
      assertEquals(result.data[2], 0);
      assertEquals(result.data[3], 255);
    }

    // Drawing after crossing the fallback threshold must still work and be
    // visible to a subsequent readback.
    ctx.fillStyle = "rgb(0, 255, 0)";
    ctx.fillRect(0, 0, 200, 200);
    const after = ctx.getImageData(10, 10, 1, 1);
    assertEquals(after.data[0], 0);
    assertEquals(after.data[1], 255);
    assertEquals(after.data[2], 0);
    assertEquals(after.data[3], 255);
  },
);

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dClipSurvivesReadbackFallbackMigration() {
    const canvas = new OffscreenCanvas(200, 200);
    const ctx = canvas.getContext("2d")!;

    ctx.beginPath();
    ctx.rect(0, 0, 100, 200);
    ctx.clip();

    // Cross the readback fallback threshold while the clip is active.
    for (let i = 0; i < 3; i++) {
      ctx.getImageData(10, 10, 1, 1);
    }

    // A full-canvas fill should still be masked by the clip after a
    // migration flattens and rebuilds the scene.
    ctx.fillStyle = "rgb(0, 0, 255)";
    ctx.fillRect(0, 0, 200, 200);
    const inside = ctx.getImageData(50, 100, 1, 1);
    assertEquals(inside.data[0], 0);
    assertEquals(inside.data[2], 255);
    const outside = ctx.getImageData(150, 100, 1, 1);
    assertEquals(outside.data[3], 0);
  },
);

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dSaveRestoreClipDepthSurvivesReadbackFallback() {
    const canvas = new OffscreenCanvas(200, 200);
    const ctx = canvas.getContext("2d")!;

    ctx.save();
    ctx.beginPath();
    ctx.rect(0, 0, 100, 200);
    ctx.clip();

    for (let i = 0; i < 3; i++) {
      ctx.getImageData(10, 10, 1, 1);
    }

    // Restoring after the migration should lift the clip on whichever
    // backend the context now runs on.
    ctx.restore();
    ctx.fillStyle = "rgb(0, 0, 255)";
    ctx.fillRect(0, 0, 200, 200);
    const outside = ctx.getImageData(150, 100, 1, 1);
    assertEquals(outside.data[0], 0);
    assertEquals(outside.data[2], 255);
    assertEquals(outside.data[3], 255);
  },
);

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dAlphaFalseStaysOpaqueAcrossReadbackFallback() {
    const canvas = new OffscreenCanvas(200, 200);
    const ctx = canvas.getContext("2d", { alpha: false })!;

    ctx.clearRect(0, 0, 200, 200);
    for (let i = 0; i < 3; i++) {
      const result = ctx.getImageData(10, 10, 1, 1);
      assertEquals(result.data[3], 255);
    }

    ctx.fillStyle = "rgba(255, 0, 0, 0)";
    ctx.fillRect(0, 0, 200, 200);
    const after = ctx.getImageData(10, 10, 1, 1);
    assertEquals(after.data[3], 255);
  },
);

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dPutImageDataAcrossReadbackThreshold() {
    const canvas = new OffscreenCanvas(200, 200);
    const ctx = canvas.getContext("2d")!;

    const imgData = ctx.createImageData(2, 2);
    for (let i = 0; i < imgData.data.length; i += 4) {
      imgData.data[i] = 255;
      imgData.data[i + 3] = 255;
    }

    for (let i = 0; i < 3; i++) {
      ctx.putImageData(imgData, 0, 0);
      const result = ctx.getImageData(0, 0, 1, 1);
      assertEquals(result.data[0], 255);
      assertEquals(result.data[3], 255);
    }
  },
);

Deno.test(
  { ignore: !hasCanvasRenderer },
  function canvas2dResizeRederivesBackendAfterFallback() {
    const canvas = new OffscreenCanvas(1, 1);
    const ctx = canvas.getContext("2d")!;

    for (let i = 0; i < 3; i++) {
      ctx.getImageData(0, 0, 1, 1);
    }

    // Growing well past the small-canvas heuristic must still leave the
    // context in a working state after `resize()` re-derives the backend.
    canvas.width = 512;
    canvas.height = 512;
    ctx.fillStyle = "rgb(255, 0, 0)";
    ctx.fillRect(0, 0, 512, 512);
    const result = ctx.getImageData(256, 256, 1, 1);
    assertEquals(result.data[0], 255);
    assertEquals(result.data[1], 0);
    assertEquals(result.data[2], 0);
    assertEquals(result.data[3], 255);
  },
);

Deno.test(
  { ignore: !hasCanvasRenderer },
  async function canvas2dConvertToBlobAcrossReadbackThreshold() {
    const canvas = new OffscreenCanvas(200, 200);
    const ctx = canvas.getContext("2d")!;

    ctx.fillStyle = "rgb(255, 0, 0)";
    ctx.fillRect(0, 0, 200, 200);
    for (let i = 0; i < 3; i++) {
      await canvas.convertToBlob({ type: "image/png" });
    }

    ctx.fillStyle = "rgb(0, 255, 0)";
    ctx.fillRect(0, 0, 200, 200);
    const result = ctx.getImageData(10, 10, 1, 1);
    assertEquals(result.data[0], 0);
    assertEquals(result.data[1], 255);
    assertEquals(result.data[2], 0);
    assertEquals(result.data[3], 255);
  },
);

Deno.test(function canvas2dRoundRectRadiusUnionSemantics() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  // DOMPointInit branch: missing/undefined members default to 0.
  ctx.roundRect(0, 0, 10, 10, [{ foo: "bar" }] as unknown as number[]);
  ctx.roundRect(0, 0, 10, 10, [[]] as unknown as number[]);
  ctx.roundRect(0, 0, 10, 10, [undefined] as unknown as number[]);
  // BigInt cannot be converted to a number.
  assertThrows(
    () => ctx.roundRect(0, 0, 10, 10, [0n] as unknown as number[]),
    TypeError,
  );
  assertThrows(
    () => ctx.roundRect(0, 0, 10, 10, [{ x: 0n }] as unknown as number[]),
    TypeError,
  );
});

Deno.test(
  { ignore: !hasCanvasRenderer },
  async function canvas2dCreateImageBitmapFromCanvas() {
    const canvas = new OffscreenCanvas(2, 2);
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = "#f00";
    ctx.fillRect(0, 0, 2, 2);
    const bitmap = await createImageBitmap(canvas);
    assertEquals(bitmap.width, 2);
    assertEquals(bitmap.height, 2);
    // @ts-ignore: Deno[Deno.internal] allowed
    const pixels: Uint8Array = Deno[Deno.internal].getBitmapData(bitmap);
    assertEquals(Array.from(pixels.subarray(0, 4)), [255, 0, 0, 255]);
    // Unlike transferToImageBitmap(), the canvas is not cleared.
    // @ts-ignore: Deno[Deno.internal] allowed
    const canvasPixels: Uint8Array = Deno[Deno.internal].getCanvasBitmapData(
      canvas,
    );
    assertEquals(Array.from(canvasPixels.subarray(0, 4)), [255, 0, 0, 255]);
  },
);

Deno.test(async function canvas2dCreateImageBitmapWithOpenLayerRejects() {
  const canvas = new OffscreenCanvas(2, 2);
  const ctx = canvas.getContext("2d")!;
  ctx.beginLayer();
  const err = await assertRejects(
    () => createImageBitmap(canvas),
    DOMException,
  );
  assertEquals(err.name, "InvalidStateError");
  ctx.endLayer();
  // Succeeds again once the layer is closed.
  const bitmap = await createImageBitmap(canvas);
  assertEquals(bitmap.width, 2);
});
