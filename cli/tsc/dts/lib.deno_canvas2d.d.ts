// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/**
 * The color type (pixel format) for the canvas rendering context.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasColorType = "float16" | "unorm8";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasDirection = "inherit" | "ltr" | "rtl";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasFontKerning = "auto" | "none" | "normal";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasFontStretch =
  | "condensed"
  | "expanded"
  | "extra-condensed"
  | "extra-expanded"
  | "normal"
  | "semi-condensed"
  | "semi-expanded"
  | "ultra-condensed"
  | "ultra-expanded";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasFontVariantCaps =
  | "all-petite-caps"
  | "all-small-caps"
  | "normal"
  | "petite-caps"
  | "small-caps"
  | "titling-caps"
  | "unicase";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasTextAlign = "center" | "end" | "left" | "right" | "start";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasTextBaseline =
  | "alphabetic"
  | "bottom"
  | "hanging"
  | "ideographic"
  | "middle"
  | "top";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasTextRendering =
  | "auto"
  | "geometricPrecision"
  | "optimizeLegibility"
  | "optimizeSpeed";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasFillRule = "evenodd" | "nonzero";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasLineCap = "butt" | "round" | "square";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasLineJoin = "bevel" | "miter" | "round";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type GlobalCompositeOperation =
  | "color"
  | "color-burn"
  | "color-dodge"
  | "copy"
  | "darken"
  | "destination-atop"
  | "destination-in"
  | "destination-out"
  | "destination-over"
  | "difference"
  | "exclusion"
  | "hard-light"
  | "hue"
  | "lighten"
  | "lighter"
  | "luminosity"
  | "multiply"
  | "overlay"
  | "saturation"
  | "screen"
  | "soft-light"
  | "source-atop"
  | "source-in"
  | "source-out"
  | "source-over"
  | "xor";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type ImageSmoothingQuality = "high" | "low" | "medium";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type FontDisplay = "auto" | "block" | "fallback" | "optional" | "swap";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type FontFaceLoadStatus = "error" | "loaded" | "loading" | "unloaded";

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type FontFaceSetLoadStatus = "loaded" | "loading";

/**
 * Settings passed as the second argument to
 * `OffscreenCanvas.getContext("2d", settings)`.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasRenderingContext2DSettings {
  /** When `false` the canvas background is always opaque black. Default: `true`. */
  alpha?: boolean;
  /**
   * Color space for the canvas. Default: `"srgb"`.
   */
  colorSpace?: PredefinedColorSpace;
  /**
   * Pixel format for the backing store. Default: `"unorm8"`.
   */
  colorType?: CanvasColorType;
  /**
   * Hint that the canvas may be updated asynchronously. Default: `false`.
   */
  desynchronized?: boolean;
  /**
   * Hint to optimize for frequent `getImageData` calls. Default: `false`.
   */
  willReadFrequently?: boolean;
}

/**
 * Metrics for a piece of text returned by
 * `OffscreenCanvasRenderingContext2D.measureText()`.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/TextMetrics
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface TextMetrics {
  /** Width of the measured text in CSS pixels. */
  readonly width: number;
  readonly actualBoundingBoxLeft: number;
  readonly actualBoundingBoxRight: number;
  readonly fontBoundingBoxAscent: number;
  readonly fontBoundingBoxDescent: number;
  readonly actualBoundingBoxAscent: number;
  readonly actualBoundingBoxDescent: number;
  readonly emHeightAscent: number;
  readonly emHeightDescent: number;
  readonly hangingBaseline: number;
  readonly alphabeticBaseline: number;
  readonly ideographicBaseline: number;
}

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
declare var TextMetrics: {
  prototype: TextMetrics;
  new (): never;
};

/**
 * A union of image source types that can be drawn onto a canvas.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
type CanvasImageSource = ImageBitmap | OffscreenCanvas;

/**
 * The **`CanvasGradient`** interface represents an opaque object describing a
 * gradient. It is returned by the methods
 * `OffscreenCanvasRenderingContext2D.createLinearGradient()`,
 * `OffscreenCanvasRenderingContext2D.createConicGradient()`, or
 * `OffscreenCanvasRenderingContext2D.createRadialGradient()`.
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasGradient)
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasGradient {
  /**
   * The **`CanvasGradient.addColorStop()`** method adds a new color stop,
   * defined by an offset and a color, to a given canvas gradient.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasGradient/addColorStop)
   */
  addColorStop(offset: number, color: string): void;
}

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
declare var CanvasGradient: {
  prototype: CanvasGradient;
  new (): CanvasGradient;
};

/**
 * The **`CanvasPattern`** interface represents an opaque object describing a
 * pattern, based on an image or a canvas, created by the
 * `OffscreenCanvasRenderingContext2D.createPattern()` method.
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasPattern)
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasPattern {
  /**
   * The **`CanvasPattern.setTransform()`** method uses a `DOMMatrix` object as
   * the pattern's transformation matrix and invokes it on the pattern.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasPattern/setTransform)
   */
  setTransform(transform?: DOMMatrix2DInit): void;
}

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
declare var CanvasPattern: {
  prototype: CanvasPattern;
  new (): CanvasPattern;
};

/**
 * The **`Path2D`** interface of the Canvas 2D API is used to declare a path
 * that can then be used on an
 * `OffscreenCanvasRenderingContext2D` object. The path methods of the
 * `OffscreenCanvasRenderingContext2D` interface are also present on this
 * interface, which gives you the convenience of being able to retain and replay
 * your path whenever desired.
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/Path2D)
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface Path2D extends CanvasPath {
  /**
   * The **`Path2D.addPath()`** method of the Canvas 2D API adds one `Path2D`
   * object to another `Path2D` object.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/Path2D/addPath)
   */
  addPath(path: Path2D, transform?: DOMMatrix2DInit): void;
}

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
declare var Path2D: {
  prototype: Path2D;
  new (path?: Path2D | string): Path2D;
};

/**
 * Compositing properties shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasCompositing {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/globalAlpha) */
  globalAlpha: number;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/globalCompositeOperation)
   */
  globalCompositeOperation: GlobalCompositeOperation;
}

/**
 * Image drawing methods shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasDrawImage {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/drawImage) */
  drawImage(image: CanvasImageSource, dx: number, dy: number): void;
  drawImage(
    image: CanvasImageSource,
    dx: number,
    dy: number,
    dw: number,
    dh: number,
  ): void;
  drawImage(
    image: CanvasImageSource,
    sx: number,
    sy: number,
    sw: number,
    sh: number,
    dx: number,
    dy: number,
    dw: number,
    dh: number,
  ): void;
}

/**
 * Path drawing methods shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasDrawPath {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/beginPath)
   */
  beginPath(): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/clip)
   */
  clip(fillRule?: CanvasFillRule): void;
  clip(path: Path2D, fillRule?: CanvasFillRule): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fill)
   */
  fill(fillRule?: CanvasFillRule): void;
  fill(path: Path2D, fillRule?: CanvasFillRule): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/isPointInPath)
   */
  isPointInPath(x: number, y: number, fillRule?: CanvasFillRule): boolean;
  isPointInPath(
    path: Path2D,
    x: number,
    y: number,
    fillRule?: CanvasFillRule,
  ): boolean;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/isPointInStroke)
   */
  isPointInStroke(x: number, y: number): boolean;
  isPointInStroke(path: Path2D, x: number, y: number): boolean;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/stroke)
   */
  stroke(): void;
  stroke(path: Path2D): void;
}

/**
 * Fill and stroke style properties shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasFillStrokeStyles {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillStyle)
   */
  fillStyle: string | CanvasGradient | CanvasPattern;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/strokeStyle)
   */
  strokeStyle: string | CanvasGradient | CanvasPattern;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createConicGradient) */
  createConicGradient(startAngle: number, x: number, y: number): CanvasGradient;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createLinearGradient) */
  createLinearGradient(
    x0: number,
    y0: number,
    x1: number,
    y1: number,
  ): CanvasGradient;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createPattern) */
  createPattern(
    image: CanvasImageSource,
    repetition: string | null,
  ): CanvasPattern | null;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createRadialGradient) */
  createRadialGradient(
    x0: number,
    y0: number,
    r0: number,
    x1: number,
    y1: number,
    r1: number,
  ): CanvasGradient;
}

/**
 * Filter property shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasFilters {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/filter)
   */
  filter: string;
}

/**
 * Image data methods shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasImageData {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createImageData) */
  createImageData(
    sw: number,
    sh: number,
    settings?: ImageDataSettings,
  ): ImageData;
  createImageData(imageData: ImageData): ImageData;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/getImageData) */
  getImageData(
    sx: number,
    sy: number,
    sw: number,
    sh: number,
    settings?: ImageDataSettings,
  ): ImageData;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/putImageData) */
  putImageData(imageData: ImageData, dx: number, dy: number): void;
  putImageData(
    imageData: ImageData,
    dx: number,
    dy: number,
    dirtyX: number,
    dirtyY: number,
    dirtyWidth: number,
    dirtyHeight: number,
  ): void;
}

/**
 * Image smoothing properties shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasImageSmoothing {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/imageSmoothingEnabled)
   */
  imageSmoothingEnabled: boolean;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/imageSmoothingQuality)
   */
  imageSmoothingQuality: ImageSmoothingQuality;
}

/**
 * Path building methods shared by the canvas rendering contexts and `Path2D`.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasPath {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/arc)
   */
  arc(
    x: number,
    y: number,
    radius: number,
    startAngle: number,
    endAngle: number,
    counterclockwise?: boolean,
  ): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/arcTo)
   */
  arcTo(x1: number, y1: number, x2: number, y2: number, radius: number): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/bezierCurveTo)
   */
  bezierCurveTo(
    cp1x: number,
    cp1y: number,
    cp2x: number,
    cp2y: number,
    x: number,
    y: number,
  ): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/closePath)
   */
  closePath(): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/ellipse)
   */
  ellipse(
    x: number,
    y: number,
    radiusX: number,
    radiusY: number,
    rotation: number,
    startAngle: number,
    endAngle: number,
    counterclockwise?: boolean,
  ): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineTo)
   */
  lineTo(x: number, y: number): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/moveTo)
   */
  moveTo(x: number, y: number): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/quadraticCurveTo)
   */
  quadraticCurveTo(cpx: number, cpy: number, x: number, y: number): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/rect)
   */
  rect(x: number, y: number, w: number, h: number): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/roundRect)
   */
  roundRect(
    x: number,
    y: number,
    w: number,
    h: number,
    radii?: number | DOMPointInit | (number | DOMPointInit)[],
  ): void;
}

/**
 * Path drawing style properties shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasPathDrawingStyles {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineCap)
   */
  lineCap: CanvasLineCap;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineDashOffset)
   */
  lineDashOffset: number;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineJoin)
   */
  lineJoin: CanvasLineJoin;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineWidth)
   */
  lineWidth: number;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/miterLimit)
   */
  miterLimit: number;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/getLineDash)
   */
  getLineDash(): number[];
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/setLineDash)
   */
  setLineDash(segments: number[]): void;
}

/**
 * Rectangle drawing methods shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasRect {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/clearRect) */
  clearRect(x: number, y: number, w: number, h: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillRect) */
  fillRect(x: number, y: number, w: number, h: number): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/strokeRect)
   */
  strokeRect(x: number, y: number, w: number, h: number): void;
}

/**
 * Shadow style properties shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasShadowStyles {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/shadowBlur)
   */
  shadowBlur: number;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/shadowColor)
   */
  shadowColor: string;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/shadowOffsetX)
   */
  shadowOffsetX: number;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/shadowOffsetY)
   */
  shadowOffsetY: number;
}

/**
 * State management methods shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasState {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/isContextLost)
   */
  isContextLost(): boolean;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/reset)
   */
  reset(): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/restore)
   */
  restore(): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/save)
   */
  save(): void;
}

/**
 * Text drawing methods shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasText {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillText) */
  fillText(text: string, x: number, y: number, maxWidth?: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/measureText) */
  measureText(text: string): TextMetrics;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/strokeText) */
  strokeText(text: string, x: number, y: number, maxWidth?: number): void;
}

/**
 * Text drawing style properties shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasTextDrawingStyles {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/direction) */
  direction: CanvasDirection;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/font) */
  font: string;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontKerning) */
  fontKerning: CanvasFontKerning;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontStretch) */
  fontStretch: CanvasFontStretch;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontVariantCaps) */
  fontVariantCaps: CanvasFontVariantCaps;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/letterSpacing) */
  letterSpacing: string;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textAlign) */
  textAlign: CanvasTextAlign;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textBaseline) */
  textBaseline: CanvasTextBaseline;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textRendering) */
  textRendering: CanvasTextRendering;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/wordSpacing) */
  wordSpacing: string;
}

/**
 * Transformation methods shared by the canvas rendering contexts.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface CanvasTransform {
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/getTransform)
   */
  getTransform(): DOMMatrix;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/resetTransform)
   */
  resetTransform(): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/rotate)
   */
  rotate(angle: number): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/scale)
   */
  scale(x: number, y: number): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/setTransform)
   */
  setTransform(
    a: number,
    b: number,
    c: number,
    d: number,
    e: number,
    f: number,
  ): void;
  setTransform(transform?: DOMMatrix2DInit): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/transform)
   */
  transform(
    a: number,
    b: number,
    c: number,
    d: number,
    e: number,
    f: number,
  ): void;
  /**
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/translate)
   */
  translate(x: number, y: number): void;
}

/**
 * A 2D rendering context for `OffscreenCanvas`.
 *
 * Obtain an instance via `OffscreenCanvas.getContext("2d")`.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/OffscreenCanvasRenderingContext2D
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface OffscreenCanvasRenderingContext2D
  extends
    CanvasCompositing,
    CanvasDrawImage,
    CanvasDrawPath,
    CanvasFillStrokeStyles,
    CanvasFilters,
    CanvasImageData,
    CanvasImageSmoothing,
    CanvasPath,
    CanvasPathDrawingStyles,
    CanvasRect,
    CanvasShadowStyles,
    CanvasState,
    CanvasText,
    CanvasTextDrawingStyles,
    CanvasTransform {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/OffscreenCanvasRenderingContext2D/canvas) */
  readonly canvas: OffscreenCanvas;
}

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
declare var OffscreenCanvasRenderingContext2D: {
  prototype: OffscreenCanvasRenderingContext2D;
  new (): never;
};

/**
 * Descriptor fields passed to the `FontFace` constructor.
 *
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface FontFaceDescriptors {
  ascentOverride?: string;
  descentOverride?: string;
  display?: FontDisplay;
  featureSettings?: string;
  lineGapOverride?: string;
  stretch?: string;
  style?: string;
  unicodeRange?: string;
  variationSettings?: string;
  weight?: string;
}

/**
 * Represents a single font face that can be loaded and added to the
 * `FontFaceSet` (`Deno.fonts`).
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FontFace
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface FontFace {
  ascentOverride: string;
  descentOverride: string;
  display: FontDisplay;
  family: string;
  featureSettings: string;
  lineGapOverride: string;
  readonly loaded: Promise<FontFace>;
  readonly status: FontFaceLoadStatus;
  stretch: string;
  style: string;
  unicodeRange: string;
  variationSettings: string;
  weight: string;
  load(): Promise<FontFace>;
}

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
declare var FontFace: {
  prototype: FontFace;
  /**
   * `ArrayBuffer` or `ArrayBufferView` instead.
   */
  new (
    family: string,
    source: BufferSource,
    descriptors?: FontFaceDescriptors,
  ): FontFace;
};

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface FontFaceSetLoadEventInit extends EventInit {
  fontfaces?: FontFace[];
}

/**
 * Event fired by `FontFaceSet` when fonts finish loading.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FontFaceSetLoadEvent
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface FontFaceSetLoadEvent extends Event {
  readonly fontfaces: ReadonlyArray<FontFace>;
}

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
declare var FontFaceSetLoadEvent: {
  prototype: FontFaceSetLoadEvent;
  new (
    type: string,
    eventInitDict?: FontFaceSetLoadEventInit,
  ): FontFaceSetLoadEvent;
};

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface FontFaceSetEventMap {
  loading: FontFaceSetLoadEvent;
  loadingdone: FontFaceSetLoadEvent;
  loadingerror: FontFaceSetLoadEvent;
}

/**
 * A set of `FontFace` objects. The global instance is `Deno.fonts`.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FontFaceSet
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
interface FontFaceSet extends EventTarget {
  onloading: ((this: FontFaceSet, ev: FontFaceSetLoadEvent) => any) | null;
  onloadingdone: ((this: FontFaceSet, ev: FontFaceSetLoadEvent) => any) | null;
  onloadingerror: ((this: FontFaceSet, ev: FontFaceSetLoadEvent) => any) | null;

  readonly size: number;
  readonly status: FontFaceSetLoadStatus;
  readonly ready: Promise<FontFaceSet>;

  add(font: FontFace): FontFaceSet;
  delete(font: FontFace): boolean;
  has(font: FontFace): boolean;
  clear(): void;
  load(font: string, text?: string): Promise<FontFace[]>;
  check(font: string, text?: string): boolean;

  forEach(
    callbackfn: (value: FontFace, key: FontFace, parent: FontFaceSet) => void,
    thisArg?: any,
  ): void;
  values(): IterableIterator<FontFace>;
  keys(): IterableIterator<FontFace>;
  entries(): IterableIterator<[FontFace, FontFace]>;
  [Symbol.iterator](): IterableIterator<FontFace>;

  addEventListener<K extends keyof FontFaceSetEventMap>(
    type: K,
    listener: (this: FontFaceSet, ev: FontFaceSetEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof FontFaceSetEventMap>(
    type: K,
    listener: (this: FontFaceSet, ev: FontFaceSetEventMap[K]) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/**
 * @experimental **UNSTABLE**: New API, yet to be vetted.
 * @category Canvas 2D
 */
declare var FontFaceSet: {
  prototype: FontFaceSet;
};
