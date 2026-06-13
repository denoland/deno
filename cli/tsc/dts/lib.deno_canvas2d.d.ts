// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Canvas 2D */
type CanvasTextAlign = "center" | "end" | "left" | "right" | "start";

/** @category Canvas 2D */
type CanvasTextBaseline =
  | "alphabetic"
  | "bottom"
  | "hanging"
  | "ideographic"
  | "middle"
  | "top";

/**
 * The predefined color spaces available for canvas rendering.
 *
 * @category Canvas 2D
 */
type PredefinedColorSpace = "display-p3" | "srgb";

/**
 * The pixel format used for the canvas backing store.
 *
 * @category Canvas 2D
 */
type CanvasColorType = "float16" | "unorm8";

/**
 * Settings passed as the second argument to
 * `OffscreenCanvas.getContext("2d", settings)`.
 *
 * @category Canvas 2D
 */
interface CanvasRenderingContext2DSettings {
  /** When `false` the canvas background is always opaque black. Default: `true`. */
  alpha?: boolean;
  /** Hint that the canvas may be updated asynchronously. Default: `false`. */
  desynchronized?: boolean;
  /** Color space for the canvas. Default: `"srgb"`. */
  colorSpace?: PredefinedColorSpace;
  /** Pixel format for the backing store. Default: `"unorm8"`. */
  colorType?: CanvasColorType;
  /** Hint to optimize for frequent `getImageData` calls. Default: `false`. */
  willReadFrequently?: boolean;
}

/**
 * Metrics for a piece of text returned by
 * `OffscreenCanvasRenderingContext2D.measureText()`.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/TextMetrics
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

/** @category Canvas 2D */
declare var TextMetrics: {
  readonly prototype: TextMetrics;
};

/**
 * A 2D rendering context for `OffscreenCanvas`.
 *
 * Obtain an instance via `OffscreenCanvas.getContext("2d")`.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/OffscreenCanvasRenderingContext2D
 * @category Canvas 2D
 */
/** @category Canvas 2D */
type CanvasDirection = "inherit" | "ltr" | "rtl";

/** @category Canvas 2D */
type CanvasFontKerning = "auto" | "none" | "normal";

/** @category Canvas 2D */
type CanvasFontStretch =
  | "ultra-condensed"
  | "extra-condensed"
  | "condensed"
  | "semi-condensed"
  | "normal"
  | "semi-expanded"
  | "expanded"
  | "extra-expanded"
  | "ultra-expanded";

/** @category Canvas 2D */
type CanvasFontVariantCaps =
  | "normal"
  | "small-caps"
  | "all-small-caps"
  | "petite-caps"
  | "all-petite-caps"
  | "unicase"
  | "titling-caps";

/** @category Canvas 2D */
type CanvasTextRendering =
  | "auto"
  | "optimizeSpeed"
  | "optimizeLegibility"
  | "geometricPrecision";

interface OffscreenCanvasRenderingContext2D {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/canvas) */
  readonly canvas: OffscreenCanvas;

  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillStyle) */
  fillStyle: string;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/strokeStyle) */
  strokeStyle: string;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/globalAlpha) */
  globalAlpha: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/font) */
  font: string;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/direction) */
  direction: CanvasDirection;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontKerning) */
  fontKerning: CanvasFontKerning;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontStretch) */
  fontStretch: CanvasFontStretch;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontVariantCaps) */
  fontVariantCaps: CanvasFontVariantCaps;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/letterSpacing) */
  letterSpacing: string;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/wordSpacing) */
  wordSpacing: string;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textAlign) */
  textAlign: CanvasTextAlign;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textBaseline) */
  textBaseline: CanvasTextBaseline;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textRendering) */
  textRendering: CanvasTextRendering;

  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillRect) */
  fillRect(x: number, y: number, w: number, h: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/clearRect) */
  clearRect(x: number, y: number, w: number, h: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillText) */
  fillText(text: string, x: number, y: number, maxWidth?: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/strokeText) */
  strokeText(text: string, x: number, y: number, maxWidth?: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/measureText) */
  measureText(text: string): TextMetrics;
}

/** @category Canvas 2D */
declare var OffscreenCanvasRenderingContext2D: {
  readonly prototype: OffscreenCanvasRenderingContext2D;
  new (): never;
};

/**
 * Descriptor fields passed to the `FontFace` constructor.
 *
 * @category Canvas 2D
 */
interface FontFaceDescriptors {
  style?: string;
  weight?: string;
  stretch?: string;
  unicodeRange?: string;
  featureSettings?: string;
  variationSettings?: string;
  display?: string;
  ascentOverride?: string;
  descentOverride?: string;
  lineGapOverride?: string;
}

/** @category Canvas 2D */
type FontFaceLoadStatus = "error" | "loaded" | "loading" | "unloaded";

/**
 * Represents a single font face that can be loaded and added to the
 * `FontFaceSet` (`Deno.fonts`).
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FontFace
 * @category Canvas 2D
 */
interface FontFace {
  family: string;
  style: string;
  weight: string;
  stretch: string;
  unicodeRange: string;
  featureSettings: string;
  variationSettings: string;
  display: string;
  ascentOverride: string;
  descentOverride: string;
  lineGapOverride: string;
  readonly status: FontFaceLoadStatus;
  readonly loaded: Promise<FontFace>;
  load(): Promise<FontFace>;
}

/** @category Canvas 2D */
declare var FontFace: {
  readonly prototype: FontFace;
  new (
    family: string,
    source: ArrayBuffer | ArrayBufferView,
    descriptors?: FontFaceDescriptors,
  ): FontFace;
};

/** @category Canvas 2D */
interface FontFaceSetLoadEventInit extends EventInit {
  fontfaces?: FontFace[];
}

/**
 * Event fired by `FontFaceSet` when fonts finish loading.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FontFaceSetLoadEvent
 * @category Canvas 2D
 */
interface FontFaceSetLoadEvent extends Event {
  readonly fontfaces: ReadonlyArray<FontFace>;
}

/** @category Canvas 2D */
declare var FontFaceSetLoadEvent: {
  readonly prototype: FontFaceSetLoadEvent;
  new (
    type: string,
    eventInitDict?: FontFaceSetLoadEventInit,
  ): FontFaceSetLoadEvent;
};

/** @category Canvas 2D */
interface FontFaceSetEventMap {
  loading: FontFaceSetLoadEvent;
  loadingdone: FontFaceSetLoadEvent;
  loadingerror: FontFaceSetLoadEvent;
}

/**
 * A set of `FontFace` objects. The global instance is `Deno.fonts`.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FontFaceSet
 * @category Canvas 2D
 */
interface FontFaceSet extends EventTarget {
  onloading: ((this: FontFaceSet, ev: FontFaceSetLoadEvent) => any) | null;
  onloadingdone: ((this: FontFaceSet, ev: FontFaceSetLoadEvent) => any) | null;
  onloadingerror: ((this: FontFaceSet, ev: FontFaceSetLoadEvent) => any) | null;

  readonly size: number;
  readonly status: "loaded" | "loading";
  readonly ready: Promise<FontFaceSet>;

  add(font: FontFace): FontFaceSet;
  delete(font: FontFace): boolean;
  has(font: FontFace): boolean;
  clear(): void;
  load(font: string, text?: string): Promise<FontFace[]>;
  check(font: string, text?: string): boolean;

  forEach(
    callbackFn: (value: FontFace, value2: FontFace, set: FontFaceSet) => void,
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

/** @category Canvas 2D */
declare var FontFaceSet: {
  readonly prototype: FontFaceSet;
};

declare namespace Deno {
  /**
   * The global set of loaded fonts, equivalent to `document.fonts` in browsers.
   *
   * Use `Deno.fonts.add(face)` to register a `FontFace` so it is available
   * for Canvas 2D text rendering.
   *
   * @category Canvas 2D
   */
  export const fonts: FontFaceSet;
}
