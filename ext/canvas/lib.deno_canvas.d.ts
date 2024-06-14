// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Canvas */
declare type ColorSpaceConversion = "default" | "none";

/** @category Canvas */
declare type ImageOrientation = "flipY" | "from-image" | "none";

/** @category Canvas */
declare type PremultiplyAlpha = "default" | "none" | "premultiply";

/** @category Canvas */
declare type ResizeQuality = "high" | "low" | "medium" | "pixelated";

/** @category Canvas */
declare type ImageBitmapSource = Blob | ImageData;

/** @category Canvas */
declare interface ImageBitmapOptions {
  colorSpaceConversion?: ColorSpaceConversion;
  imageOrientation?: ImageOrientation;
  premultiplyAlpha?: PremultiplyAlpha;
  resizeHeight?: number;
  resizeQuality?: ResizeQuality;
  resizeWidth?: number;
}

/** @category Canvas */
declare function createImageBitmap(
  image: ImageBitmapSource,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;
/** @category Canvas */
declare function createImageBitmap(
  image: ImageBitmapSource,
  sx: number,
  sy: number,
  sw: number,
  sh: number,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;

/** @category Canvas */
declare interface ImageBitmap {
  readonly height: number;
  readonly width: number;
  close(): void;
}

/** @category Canvas */
declare var ImageBitmap: {
  prototype: ImageBitmap;
  new (): ImageBitmap;
};
