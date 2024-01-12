// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Web APIs */
type PredefinedColorSpace = "srgb" | "display-p3";

/** @category Web APIs */
interface ImageDataSettings {
  readonly colorSpace?: PredefinedColorSpace;
}

/** @category Web APIs */
interface ImageData {
  readonly colorSpace: PredefinedColorSpace;
  readonly data: Uint8ClampedArray;
  readonly height: number;
  readonly width: number;
}

/** @category Web APIs */
declare var ImageData: {
  prototype: ImageData;
  new (sw: number, sh: number, settings?: ImageDataSettings): ImageData;
  new (
    data: Uint8ClampedArray,
    sw: number,
    sh?: number,
    settings?: ImageDataSettings,
  ): ImageData;
};

/** @category Web APIs */
declare type ImageBitmapSource = Blob | ImageData;

/** @category Web APIs */
interface ImageBitmapOptions {
  colorSpaceConversion?: ColorSpaceConversion;
  imageOrientation?: ImageOrientation;
  premultiplyAlpha?: PremultiplyAlpha;
  resizeHeight?: number;
  resizeQuality?: ResizeQuality;
  resizeWidth?: number;
}

/** @category Web APIs */
declare function createImageBitmap(
  image: ImageBitmapSource,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;
/** @category Web APIs */
declare function createImageBitmap(
  image: ImageBitmapSource,
  sx: number,
  sy: number,
  sw: number,
  sh: number,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;
