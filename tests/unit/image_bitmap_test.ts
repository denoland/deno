// Copyright 2018-2025 the Deno authors. MIT license.

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

Deno.test(async function imageBitmapRecivesImageBitmap() {
  const imageData = new Blob(
    [await Deno.readFile(`${prefix}/1x1-red16.png`)],
    { type: "image/png" },
  );
  const imageBitmap1 = await createImageBitmap(imageData);
  const imageBitmap2 = await createImageBitmap(imageBitmap1);
  assertEquals(
    // @ts-ignore: Deno[Deno.internal].core allowed
    Deno[Deno.internal].getBitmapData(imageBitmap1),
    // @ts-ignore: Deno[Deno.internal].core allowed
    Deno[Deno.internal].getBitmapData(imageBitmap2),
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
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 1, 0, 0, 1
  ]));
});

Deno.test(async function imageBitmapCropGreater() {
  const data = generateNumberedData(3 * 3);
  const imageData = new ImageData(data, 3, 3);
  const imageBitmap = await createImageBitmap(imageData, -1, -1, 5, 5);
  // @ts-ignore: Deno[Deno.internal].core allowed
  // deno-fmt-ignore
  assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 1, 0, 0, 1, 2, 0, 0, 1, 3, 0, 0, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 4, 0, 0, 1, 5, 0, 0, 1, 6, 0, 0, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 7, 0, 0, 1, 8, 0, 0, 1, 9, 0, 0, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
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
    1, 0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 1, 3, 0, 0, 1, 3, 0, 0, 1,
    1, 0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 1, 3, 0, 0, 1, 3, 0, 0, 1,
    1, 0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 1, 3, 0, 0, 1, 3, 0, 0, 1,
    1, 0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 1, 3, 0, 0, 1, 3, 0, 0, 1,
    1, 0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 1, 3, 0, 0, 1, 3, 0, 0, 1
  ]));
});

Deno.test("imageOrientation", async (t) => {
  await t.step('"ImageData" imageOrientation: "flipY"', async () => {
    const data = generateNumberedData(9);
    const imageData = new ImageData(data, 3, 3);
    const imageBitmap = await createImageBitmap(imageData, {
      imageOrientation: "flipY",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
      7, 0, 0, 1, 8, 0, 0, 1, 9, 0, 0, 1,
      4, 0, 0, 1, 5, 0, 0, 1, 6, 0, 0, 1,
      1, 0, 0, 1, 2, 0, 0, 1, 3, 0, 0, 1,
    ]));
  });

  const imageData = new Blob(
    [await Deno.readFile(`${prefix}/squares_6.jpg`)],
    { type: "image/jpeg" },
  );
  const WIDTH = 320;
  const CHANNELS = 3;
  const TARGET_PIXEL_X = 40;
  const START = TARGET_PIXEL_X * WIDTH * CHANNELS;
  const END = START + CHANNELS;
  // reference:
  // https://github.com/web-platform-tests/wpt/blob/a1f4bbf4c6e1a9a861a145a34cd097ea260b5a49/html/canvas/element/manual/imagebitmap/createImageBitmap-exif-orientation.html#L30
  await t.step('"Blob" imageOrientation: "from-image"', async () => {
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    const targetPixel = Deno[Deno.internal].getBitmapData(imageBitmap).slice(
      START,
      END,
    );
    assertEquals(targetPixel, new Uint8Array([253, 0, 0]));
  });
  // reference:
  // https://github.com/web-platform-tests/wpt/blob/a1f4bbf4c6e1a9a861a145a34cd097ea260b5a49/html/canvas/element/manual/imagebitmap/createImageBitmap-exif-orientation.html#L55
  await t.step('"Blob" imageOrientation: "flipY"', async () => {
    const imageBitmap = await createImageBitmap(imageData, {
      imageOrientation: "flipY",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    const targetPixel = Deno[Deno.internal].getBitmapData(imageBitmap).slice(
      START,
      END,
    );
    assertEquals(targetPixel, new Uint8Array([253, 127, 127]));
  });
});

Deno.test("imageBitmapPremultiplyAlpha", async (t) => {
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
  await t.step('"ImageData" premultiplyAlpha: "default"', async () => {
    const imageBitmap = await createImageBitmap(imageData, {
      premultiplyAlpha: "default",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
      255, 255, 0, 153,
    ]));
  });
  await t.step('"ImageData" premultiplyAlpha: "premultiply"', async () => {
    const imageBitmap = await createImageBitmap(imageData, {
      premultiplyAlpha: "premultiply",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
      153, 153, 0, 153
    ]));
  });
  await t.step('"ImageData" premultiplyAlpha: "none"', async () => {
    const imageBitmap = await createImageBitmap(imageData, {
      premultiplyAlpha: "none",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([
      255, 255, 0, 153,
    ]));
  });
  await t.step('"Blob" premultiplyAlpha: "none"', async () => {
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
      255, 0, 0, 255, 0, 255, 0, 255,
      0, 0, 255, 255, 255, 0, 0, 127
    ]));
  });
});

Deno.test("imageBitmapFromBlob", async (t) => {
  await t.step("8-bit png", async () => {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.png`)],
      { type: "image/png" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  });
  await t.step("16-bit png", async () => {
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
          0, 0, // G
          0, 0, // B
          255, 255  // A
        ]
      )
    );
  });
  await t.step("8-bit jpeg", async () => {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.jpeg`)],
      { type: "image/jpeg" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([254, 0, 0]));
  });
  await t.step("8-bit bmp", async () => {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.bmp`)],
      { type: "image/bmp" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  });
  await t.step("8-bit gif", async () => {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.gif`)],
      { type: "image/gif" },
    );
    await assertRejects(() => createImageBitmap(imageData), DOMException);
    // TODO(Hajime-san): remove the comment out when the implementation is ready
    // const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    // assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  });
  await t.step("8-bit webp", async () => {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.webp`)],
      { type: "image/webp" },
    );
    await assertRejects(() => createImageBitmap(imageData), DOMException);
    // TODO(Hajime-san): remove the comment out when the implementation is ready
    // const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    // assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  });
  await t.step("8-bit ico", async () => {
    const imageData = new Blob(
      [await Deno.readFile(`${prefix}/1x1-red8.ico`)],
      { type: "image/x-icon" },
    );
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  });
  await t.step("flotat-32-bit exr", async () => {
    // image/x-exr is a known mimetype for OpenEXR
    // https://www.digipres.org/formats/sources/fdd/formats/#fdd000583
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/1x1-red32f.exr`),
    ], { type: "image/x-exr" });
    await assertRejects(() => createImageBitmap(imageData), DOMException);
  });
});

Deno.test("imageBitmapFromBlobAnimatedImage", async (t) => {
  await t.step("animated png has a default image", async () => {
    // the chunk of animated apng is below (2 frames, 1x1, 8-bit, RGBA), default [255, 0, 0, 255] image
    // [ 0, 255, 0, 255,
    //   0, 0, 255, 255 ]
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/1x1-2f-animated-has-def.png`),
    ], { type: "image/png" });
    const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  });
  await t.step("animated png does not have any default image", async () => {
    // the chunk of animated apng is below (3 frames, 1x1, 8-bit, RGBA)
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
  });
  await t.step("animated webp", async () => {
    // the chunk of animated webp is below (3 frames, 1x1, 8-bit, RGBA)
    //
    // [ 255, 0, 0, 127,
    //   0, 255, 0, 127,
    //   0, 0, 255, 127 ]
    const imageData = new Blob([
      await Deno.readFile(
        `${prefix}/1x1-3f-lossless-animated-semi-transparent.webp`,
      ),
    ], { type: "image/webp" });
    await assertRejects(() => createImageBitmap(imageData), DOMException);
    // TODO(Hajime-san): remove the comment out when the implementation is ready
    // const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    // assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 127]));
  });
  await t.step("animated gif", async () => {
    // the chunk of animated gif is below (3 frames, 1x1, 8-bit, RGBA)
    // [ 255, 0, 0, 255,
    //   0, 255, 0, 255,
    //   0, 0, 255, 255 ]
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/1x1-3f-animated.gif`),
    ], { type: "image/gif" });
    await assertRejects(() => createImageBitmap(imageData), DOMException);
    // TODO(Hajime-san): remove the comment out when the implementation is ready
    // const imageBitmap = await createImageBitmap(imageData);
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    // assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255, 0, 0, 255]));
  });
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

Deno.test("imageBitmapFromBlobColorspaceConversion", async (t) => {
  // reference:
  // https://github.com/web-platform-tests/wpt/blob/d575dc75ede770df322fbc5da3112dcf81f192ec/html/canvas/element/manual/imagebitmap/createImageBitmap-colorSpaceConversion.html#L18
  // https://wpt.fyi/results/html/canvas/element/manual/imagebitmap/createImageBitmap-colorSpaceConversion.html?label=experimental&label=master&aligned
  await t.step('"Blob" colorSpaceConversion: "none"', async () => {
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/wide-gamut-pattern.png`),
    ], { type: "image/png" });
    const imageBitmap = await createImageBitmap(imageData, {
      colorSpaceConversion: "none",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    const firstPixel = extractHighBytes(Deno[Deno.internal].getBitmapData(imageBitmap)).slice(0, 4);
    // picking the high bytes of the first pixel
    assertEquals(firstPixel, new Uint8Array([123, 0, 27, 255]));
  });
  await t.step('"Blob" colorSpaceConversion: "default"', async () => {
    const imageData = new Blob([
      await Deno.readFile(`${prefix}/wide-gamut-pattern.png`),
    ], { type: "image/png" });
    const imageBitmap = await createImageBitmap(imageData, {
      colorSpaceConversion: "default",
    });
    // @ts-ignore: Deno[Deno.internal].core allowed
    // deno-fmt-ignore
    const firstPixel = extractHighBytes(Deno[Deno.internal].getBitmapData(imageBitmap)).slice(0, 4);
    // picking the high bytes of the first pixel
    assertEquals(firstPixel, new Uint8Array([255, 0, 0, 255]));
  });
});
