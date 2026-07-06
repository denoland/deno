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
  async function loadLocalFontsSucceeds() {
    await Deno.loadLocalFonts();
  },
);

Deno.test(
  { permissions: { sys: [] } },
  async function loadLocalFontsRequiresPermission() {
    await assertRejects(
      () => Deno.loadLocalFonts(),
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

Deno.test(function canvasFilterConstructorValidates() {
  new CanvasFilter({ name: "gaussianBlur", stdDeviation: 5 });
  new CanvasFilter({ name: "gaussianBlur", stdDeviation: [1, 2] });
  new CanvasFilter([
    { name: "gaussianBlur", stdDeviation: 5 },
    { name: "dropShadow", dx: 10, dy: 10 },
  ]);
  new CanvasFilter({ name: "convolveMatrix", kernelMatrix: [[1]] });
  new CanvasFilter({ name: "dropShadow", floodColor: "canvas" });
  new CanvasFilter({ name: "turbulence", stitchTiles: "stitch" });
  // Unknown filter names are tolerated.
  new CanvasFilter({ name: "unknownFilter" });

  assertThrows(() => new CanvasFilter({ name: "gaussianBlur" }), TypeError);
  assertThrows(
    () => new CanvasFilter({ name: "gaussianBlur", stdDeviation: [1, 2, 3] }),
    TypeError,
  );
  assertThrows(
    () => new CanvasFilter({ name: "colorMatrix", values: [1, 2, 3] }),
    TypeError,
  );
  assertThrows(
    () => new CanvasFilter({ name: "convolveMatrix", kernelMatrix: [[], []] }),
    TypeError,
  );
  assertThrows(
    () => new CanvasFilter({ name: "dropShadow", dx: NaN }),
    TypeError,
  );
  assertThrows(
    () => new CanvasFilter({ name: "dropShadow", floodColor: "not-a-color" }),
    TypeError,
  );
  assertThrows(
    () => new CanvasFilter({ name: "turbulence", stitchTiles: "yes" }),
    TypeError,
  );
});

Deno.test(function canvasFilterPropertyUnion() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertStrictEquals(ctx.filter, "none");

  ctx.filter = "blur(5px)";
  assertStrictEquals(ctx.filter, "blur(5px)");

  const filter = new CanvasFilter({ name: "gaussianBlur", stdDeviation: 5 });
  ctx.filter = filter;
  assertStrictEquals(ctx.filter, filter);
  assertStrictEquals(
    Object.prototype.toString.call(ctx.filter),
    "[object CanvasFilter]",
  );

  // An invalid filter string leaves the current (object) value in place.
  ctx.filter = "this string is not a filter";
  assertStrictEquals(ctx.filter, filter);

  ctx.filter = "none";
  assertStrictEquals(ctx.filter, "none");
});

Deno.test(function canvas2dBeginLayerOptionsValidated() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  ctx.beginLayer();
  ctx.endLayer();
  ctx.beginLayer(null);
  ctx.endLayer();
  ctx.beginLayer({ filter: { name: "unknownFilter" } });
  ctx.endLayer();
  ctx.beginLayer({ filter: "invalid filter strings are tolerated" });
  ctx.endLayer();

  // deno-lint-ignore no-explicit-any
  assertThrows(() => ctx.beginLayer("" as any), TypeError);
  assertThrows(
    () => ctx.beginLayer({ filter: { name: "gaussianBlur" } }),
    TypeError,
  );
});

Deno.test(function canvas2dGetImageDataTooLargeThrows() {
  const ctx = new OffscreenCanvas(10, 10).getContext("2d")!;
  assertThrows(() => ctx.getImageData(0, 0, 2147483647, 10), TypeError);
  assertThrows(() => ctx.createImageData(2147483647, 10), TypeError);
});

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
