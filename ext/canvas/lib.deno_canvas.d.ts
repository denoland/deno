// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/**
 * Specifies whether the image should be decoded using color space conversion.
 * Either none or default (default). The value default indicates that
 * implementation-specific behavior is used.
 *
 * @category Canvas
 */
type ColorSpaceConversion = "default" | "none";

/**
 * Specifies how the bitmap image should be oriented.
 *
 * @category Canvas
 */
type ImageOrientation = "flipY" | "from-image" | "none";

/**
 * Specifies whether the bitmap's color channels should be premultiplied by
 * the alpha channel.
 *
 * @category Canvas
 */
type PremultiplyAlpha = "default" | "none" | "premultiply";

/**
 * Specifies the algorithm to be used for resizing the input to match the
 * output dimensions. One of `pixelated`, `low` (default), `medium`, or `high`.
 *
 * @category Canvas
 */
type ResizeQuality = "high" | "low" | "medium" | "pixelated";

/**
 * The `ImageBitmapSource` type represents an image data source that can be
 * used to create an `ImageBitmap`.
 *
 * @category Canvas */
type ImageBitmapSource = Blob | ImageData;

/**
 * The options of {@linkcode createImageBitmap}.
 *
 * @category Canvas */
interface ImageBitmapOptions {
  /**
   * Specifies whether the image should be decoded using color space
   * conversion. Either none or default (default). The value default
   * indicates that implementation-specific behavior is used.
   */
  colorSpaceConversion?: ColorSpaceConversion;
  /** Specifies how the bitmap image should be oriented. */
  imageOrientation?: ImageOrientation;
  /**
   * Specifies whether the bitmap's color channels should be premultiplied
   * by the alpha channel. One of none, premultiply, or default (default).
   */
  premultiplyAlpha?: PremultiplyAlpha;
  /** The output height. */
  resizeHeight?: number;
  /**
   * Specifies the algorithm to be used for resizing the input to match the
   * output dimensions. One of pixelated, low (default), medium, or high.
   */
  resizeQuality?: ResizeQuality;
  /** The output width. */
  resizeWidth?: number;
}

/**
 * Create a new {@linkcode ImageBitmap} object from a given source.
 *
 * @param image The image to create an {@linkcode ImageBitmap} from.
 * @param options The options for creating the {@linkcode ImageBitmap}.
 *
 * @category Canvas
 */
declare function createImageBitmap(
  image: ImageBitmapSource,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;
/**
 * Create a new {@linkcode ImageBitmap} object from a given source, cropping
 * to the specified rectangle.
 *
 * @param image The image to create an {@linkcode ImageBitmap} from.
 * @param sx The x coordinate of the top-left corner of the sub-rectangle from
 *           which the {@linkcode ImageBitmap} will be cropped.
 * @param sy The y coordinate of the top-left corner of the sub-rectangle from
 *           which the {@linkcode ImageBitmap} will be cropped.
 * @param sw The width of the sub-rectangle from which the
 *           {@linkcode ImageBitmap} will be cropped.
 * @param sh The height of the sub-rectangle from which the
 *           {@linkcode ImageBitmap} will be cropped.
 * @param options The options for creating the {@linkcode ImageBitmap}.
 *
 * @category Canvas
 */
declare function createImageBitmap(
  image: ImageBitmapSource,
  sx: number,
  sy: number,
  sw: number,
  sh: number,
  options?: ImageBitmapOptions,
): Promise<ImageBitmap>;

/**
 * `ImageBitmap` interface represents a bitmap image which can be drawn to a canvas.
 *
 * @category Canvas
 */
interface ImageBitmap {
  /**
   * The height of the bitmap.
   */
  readonly height: number;
  /**
   * The width of the bitmap.
   */
  readonly width: number;
  /**
   * Releases imageBitmap's resources.
   */
  close(): void;
}

/**
 * `ImageBitmap` represents a bitmap image which can be drawn to a canvas.
 *
 * @category Canvas
 */
declare var ImageBitmap: {
  prototype: ImageBitmap;
  new (): ImageBitmap;
};
