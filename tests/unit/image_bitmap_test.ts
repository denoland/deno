// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "./test_util.ts";

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

Deno.test(async function imageBitmapFromBlob() {
  const path = "tests/testdata/image/1x1-white.png";
  const imageData = new Blob([await Deno.readFile(path)], {
    type: "image/png",
  });
  const imageBitmap = await createImageBitmap(imageData);
  // @ts-ignore: Deno[Deno.internal].core allowed
  // deno-fmt-ignore
  assertEquals(Deno[Deno.internal].getBitmapData(imageBitmap), new Uint8Array([255,255,255,255]));
});
