// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "./test_util.ts";

Deno.test(function imageDataInitializedWithSourceWidthAndHeight() {
  const imageData = new ImageData(16, 9);

  assertEquals(imageData.width, 16);
  assertEquals(imageData.height, 9);
  assertEquals(imageData.data.length, 16 * 9 * 4); // width * height * 4 (RGBA pixels)
  assertEquals(imageData.colorSpace, "srgb");
});

Deno.test(function imageDataInitializedWithImageDataAndWidth() {
  const imageData = new ImageData(new Uint8ClampedArray(16 * 9 * 4), 16);

  assertEquals(imageData.width, 16);
  assertEquals(imageData.height, 9);
  assertEquals(imageData.data.length, 16 * 9 * 4); // width * height * 4 (RGBA pixels)
  assertEquals(imageData.colorSpace, "srgb");
});

Deno.test(
  function imageDataInitializedWithImageDataAndWidthAndHeightAndColorSpace() {
    const imageData = new ImageData(new Uint8ClampedArray(16 * 9 * 4), 16, 9, {
      colorSpace: "display-p3",
    });

    assertEquals(imageData.width, 16);
    assertEquals(imageData.height, 9);
    assertEquals(imageData.data.length, 16 * 9 * 4); // width * height * 4 (RGBA pixels)
    assertEquals(imageData.colorSpace, "display-p3");
  },
);

Deno.test(
  async function imageDataUsedInWorker() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const url = import.meta.resolve(
      "../testdata/workers/image_data_worker.ts",
    );
    const expectedData = 16;

    const worker = new Worker(url, { type: "module" });
    worker.onmessage = function (e) {
      assertEquals(expectedData, e.data);
      worker.terminate();
      resolve();
    };

    await promise;
  },
);
