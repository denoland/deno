// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertRejects } from "./test_util.ts";

const prefix = "tests/testdata/image";

function generateNumberedData(n: number): Uint8ClampedArray {
  return new Uint8ClampedArray(
    Array.from({ length: n }, (_, i) => [i + 1, 0, 0, 1]).flat(),
  );
}

Deno.test(async function imageBitmapDirect() {
  const data = generateNumberedData(3);
  const imageData = new ImageData(data, 3, 1);
  const imageBitmap = await createImageBitmap(imageData);
  assertEquals(
    // @ts-ignore: Deno[Deno.internal].core allowed
    Deno[Deno.internal].getBitmapData(imageBitmap),
    new Uint8Array(data.buffer),
  );
});

Deno.test(async function imageBitmapCrop() {
  const data = generateNumberedData(3 * 3);
  const imageData = new ImageData(data, 3, 3);
  const imageBitmap = await createImageBitmap(imageData, 1, 1, 1, 1);
  assertEquals(
    // @ts-ignore: Deno[Deno.internal].core allowed
    Deno[Deno.internal].getBitmapData(imageBitmap),
    new Uint8Array([5, 0, 0, 1]),
  );
});

Deno.test(async function imageBitmapCropPartialNegative() {
  const data = generateNumberedData(3 * 3);
  const imageData = new ImageData(data, 3, 3);
  const imageBitmap = await createImageBitmap(imageData, -1, -1, 2, 2);
  // @ts-ignore: Deno[Deno.internal].core allowed
  // deno-fmt-ignore
  assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
    0, 0, 0, 0,   0, 0, 0, 0,
    0, 0, 0, 0,   1, 0, 0, 1
  ]));
});

Deno.test(async function imageBitmapCropGreater() {
  const data = generateNumberedData(3 * 3);
  const imageData = new ImageData(data, 3, 3);
  const imageBitmap = await createImageBitmap(imageData, -1, -1, 5, 5);
  // @ts-ignore: Deno[Deno.internal].core allowed
  // deno-fmt-ignore
  assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
    0, 0, 0, 0,   0, 0, 0, 0,   0, 0, 0, 0,   0, 0, 0, 0,   0, 0, 0, 0,
    0, 0, 0, 0,   1, 0, 0, 1,   2, 0, 0, 1,   3, 0, 0, 1,   0, 0, 0, 0,
    0, 0, 0, 0,   4, 0, 0, 1,   5, 0, 0, 1,   6, 0, 0, 1,   0, 0, 0, 0,
    0, 0, 0, 0,   7, 0, 0, 1,   8, 0, 0, 1,   9, 0, 0, 1,   0, 0, 0, 0,
    0, 0, 0, 0,   0, 0, 0, 0,   0, 0, 0, 0,   0, 0, 0, 0,   0, 0, 0, 0,
  ]));
});

Deno.test(async function imageBitmapScale() {
  const data = generateNumberedData(3);
  const imageData = new ImageData(data, 3, 1);
  const imageBitmap = await createImageBitmap(imageData, {
    resizeHeight: 5,
    resizeWidth: 5,
    resizeQuality: "pixelated",
  });
  // @ts-ignore: Deno[Deno.internal].core allowed
  // deno-fmt-ignore
  assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
    1, 0, 0, 1,   1, 0, 0, 1,   2, 0, 0, 1,   3, 0, 0, 1,   3, 0, 0, 1,
    1, 0, 0, 1,   1, 0, 0, 1,   2, 0, 0, 1,   3, 0, 0, 1,   3, 0, 0, 1,
    1, 0, 0, 1,   1, 0, 0, 1,   2, 0, 0, 1,   3, 0, 0, 1,   3, 0, 0, 1,
    1, 0, 0, 1,   1, 0, 0, 1,   2, 0, 0, 1,   3, 0, 0, 1,   3, 0, 0, 1,
    1, 0, 0, 1,   1, 0, 0, 1,   2, 0, 0, 1,   3, 0, 0, 1,   3, 0, 0, 1
  ]));
});

Deno.test(async function imageBitmapFlipY() {
  const data = generateNumberedData(9);
  const imageData = new ImageData(data, 3, 3);
  const imageBitmap = await createImageBitmap(imageData, {
    imageOrientation: "flipY",
  });
  // @ts-ignore: Deno[Deno.internal].core allowed
  // deno-fmt-ignore
  assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
    7, 0, 0, 1,   8, 0, 0, 1,   9, 0, 0, 1,
    4, 0, 0, 1,   5, 0, 0, 1,   6, 0, 0, 1,
    1, 0, 0, 1,   2, 0, 0, 1,   3, 0, 0, 1,
  ]));
});

Deno.test(async function imageBitmapPremultiplyAlpha() {
  const imageData = new ImageData(
    new Uint8ClampedArray([
      255,
      255,
      0,
      153,
    ]),
    1,
    1,
  );
  {
    const imageBitmap = await createImageBitmap(imageData, {
      premultiplyAlpha: "default",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
      255, 255, 0, 153,
    ]));
  }
  {
    const imageBitmap = await createImageBitmap(imageData, {
      premultiplyAlpha: "premultiply",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
      153, 153, 0, 153
    ]));
  }
  {
    const imageBitmap = await createImageBitmap(imageData, {
      premultiplyAlpha: "none",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
      255, 255, 0, 153,
    ]));
  }
  {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/2x2-transparent8.png`)],
      { type: "image/png" },
    );
    const imageBitmap = await createImageBitmap(imageData, {
      premultiplyAlpha: "none",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
      255, 0, 0, 255,   0, 255, 0, 255,
      0, 0, 255, 255,   255, 0, 0, 127
    ]));
  }
});

Deno.test(async function imageBitmapFromBlob() {
  {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.png`)],
      { type: "image/png" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  }
  {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red16.png`)],
      { type: "image/png" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap),
      // deno-fmt-ignore
      new Uint8Array(
        [
          255, 255, // R
            0,   0, // G
            0,   0, // B
          255, 255  // A
        ]
      )
    );
  }
  {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.jpeg`)],
      { type: "image/jpeg" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([254, 0, 0]));
  }
  {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.bmp`)],
      { type: "image/bmp" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  }
  {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.gif`)],
      { type: "image/gif" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  }
  {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.webp`)],
      { type: "image/webp" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  }
  {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.ico`)],
      { type: "image/x-icon" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  }
  {
    // image/x-exr is a known mimetype for OpenEXR
    // https://www.digipres.org/formats/sources/fdd/formats/#fdd000583
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/1x1-red32f.exr`),
    ], { type: "image/x-exr" });
    await assertRejects(() => createImageBitmap(imageData), DOMException);
  }
});

Deno.test(async function imageBitmapFromBlobAnimatedImage() {
  {
    // the chunk of animated apng is below (2 frames, 1x1, 8-bit, RGBA), has default [255, 0, 0, 255] image
    // [ 0, 255, 0, 255,
    //   0, 0, 255, 255 ]
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/1x1-2f-animated-has-def.png`),
    ], { type: "image/png" });
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  }
  {
    // the chunk of animated apng is below (3 frames, 1x1, 8-bit, RGBA), no default image
    // [ 255, 0, 0, 255,
    //   0, 255, 0, 255,
    //   0, 0, 255, 255 ]
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/1x1-3f-animated-no-def.png`),
    ], { type: "image/png" });
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  }
  {
    // the chunk of animated webp is below (3 frames, 1x1, 8-bit, RGBA)
    //
    // [ 255, 0, 0, 127,
    //   0, 255, 0, 127,
    //   0, 0, 255, 127 ]

    // the command to generate the webp file
    // % img2webp -loop 0 0.png 1.png 2.png -o out.webp -o out.webp
    // https://developers.google.com/speed/webp/docs/img2webp

    // deno % webpinfo tests/testdata/image/1x1-3f-lossless-animated-semi-transparent.webp
    // File: tests/testdata/image/1x1-3f-lossless-animated-semi-transparent.webp
    // RIFF HEADER:
    //   File size:    188
    // Chunk VP8X at offset     12, length     18
    //   ICCP: 0
    //   Alpha: 1
    //   EXIF: 0
    //   XMP: 0
    //   Animation: 1
    //   Canvas size 1 x 1
    // Chunk ANIM at offset     30, length     14
    //   Background color:(ARGB) ff ff ff ff
    //   Loop count      : 0
    // Chunk ANMF at offset     44, length     48
    //   Offset_X: 0
    //   Offset_Y: 0
    //   Width: 1
    //   Height: 1
    //   Duration: 100
    //   Dispose: 0
    //   Blend: 1
    // Chunk VP8L at offset     68, length     24
    //   Width: 1
    //   Height: 1
    //   Alpha: 1
    //   Animation: 0
    //   Format: Lossless (2)
    // Chunk ANMF at offset     92, length     48
    //   Offset_X: 0
    //   Offset_Y: 0
    //   Width: 1
    //   Height: 1
    //   Duration: 100
    //   Dispose: 0
    //   Blend: 1
    // Chunk VP8L at offset    116, length     24
    //   Width: 1
    //   Height: 1
    //   Alpha: 1
    //   Animation: 0
    //   Format: Lossless (2)
    // Chunk ANMF at offset    140, length     48
    //   Offset_X: 0
    //   Offset_Y: 0
    //   Width: 1
    //   Height: 1
    //   Duration: 100
    //   Dispose: 0
    //   Blend: 1
    // Chunk VP8L at offset    164, length     24
    //   Width: 1
    //   Height: 1
    //   Alpha: 1
    //   Animation: 0
    //   Format: Lossless (2)
    // No error detected.

    const imageData = new Blob([
      await Deno.readFile(
        `${prefix}/1x1-3f-lossless-animated-semi-transparent.webp`,
      ),
    ], { type: "image/webp" });
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 127]));
  }
  {
    // the chunk of animated gif is below (3 frames, 1x1, 8-bit, RGBA)
    // [ 255, 0, 0, 255,
    //   0, 255, 0, 255,
    //   0, 0, 255, 255 ]
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/1x1-3f-animated.gif`),
    ], { type: "image/gif" });
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  }
});

Deno.test(async function imageBitmapImageDataColorspaceConversion() {
  {
    const imageData = new ImageData(
      new Uint8ClampedArray([
        255,
        0,
        0,
        255,
      ]),
      1,
      1,
      {
        colorSpace: "display-p3",
      },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([234, 51, 35, 255]));
  }
});

/**
 * extract high bytes from Uint16Array
 */
function extractHighBytes(array: Uint8Array): Uint8Array {
  const highBytes = new Uint8Array(array.length / 2);
  for (let i = 0, j = 1; i < array.length; i++, j += 2) {
    highBytes[i] = array[j];
  }
  return highBytes;
}

Deno.test(async function imageBitmapFromBlobColorspaceConversion() {
  // reference:
  // https://github.com/web-platform-tests/wpt/blob/d575dc75ede770df322fbc5da3112dcf81f192ec/html/canvas/element/manual/imagebitmap/createImageBitmap-colorSpaceConversion.html#L18
  // https://wpt.fyi/results/html/canvas/element/manual/imagebitmap/createImageBitmap-colorSpaceConversion.html?label=experimental&label=master&aligned
  {
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/wide-gamut-pattern.png`),
    ], { type: "image/png" });
    const imageBitmap = await createImageBitmap(imageData, {
      colorSpaceConversion: "none",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    const firstPixel = extractHighBytes(Deno[Deno.internal].getBitmapData(imageBitmap),).slice(0, 4);
    assertEquals(firstPixel, new Uint8Array([123, 0, 27, 255]));
  }
  {
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/wide-gamut-pattern.png`),
    ], { type: "image/png" });
    const imageBitmap = await createImageBitmap(imageData, {
      colorSpaceConversion: "default",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    const firstPixel = extractHighBytes(Deno[Deno.internal].getBitmapData(imageBitmap),).slice(0, 4);
    assertEquals(firstPixel, new Uint8Array([255, 0, 0, 255]));
  }
});
