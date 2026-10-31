// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, assertThrows } from "./test_util.ts";

function imageBitmapData(bitmap: ImageBitmap) {
  return (bitmap as unknown as Record<symbol, () => Uint8Array>)[
    Symbol.for("Deno_bitmapData")
  ]();
}

// Basic tests for the structured clone algorithm. Mainly tests TypeScript
// typings. Actual functionality is tested in WPT.

Deno.test("self.structuredClone", async () => {
  const arrayOriginal = ["hello world"];
  const channelOriginal = new MessageChannel();
  const [arrayCloned, portTransferred] = self
    .structuredClone(
      [arrayOriginal, channelOriginal.port2] as [string[], MessagePort],
      {
        transfer: [channelOriginal.port2],
      },
    );
  assert(arrayOriginal !== arrayCloned); // not the same identity
  assertEquals(arrayCloned, arrayOriginal); // but same value
  channelOriginal.port1.postMessage("1");
  await new Promise((resolve) => portTransferred.onmessage = () => resolve(1));
  channelOriginal.port1.close();
  portTransferred.close();
});

Deno.test("correct DataCloneError message", () => {
  assertThrows(
    () => {
      const sab = new SharedArrayBuffer(1024);
      structuredClone(sab, {
        // @ts-expect-error cannot assign SharedArrayBuffer because it's not tranferable
        transfer: [sab],
      });
    },
    DOMException,
    "Value not transferable",
  );

  const ab = new ArrayBuffer(1);
  // detach ArrayBuffer
  structuredClone(ab, { transfer: [ab] });
  assertThrows(
    () => {
      structuredClone(ab, { transfer: [ab] });
    },
    DOMException,
    "ArrayBuffer at index 0 is already detached",
  );

  const ab2 = new ArrayBuffer(0);
  assertThrows(
    () => {
      structuredClone([ab2, ab], { transfer: [ab2, ab] });
    },
    DOMException,
    "ArrayBuffer at index 1 is already detached",
  );

  // ab2 should not be detached after above failure
  structuredClone(ab2, { transfer: [ab2] });
});

Deno.test("structuredClone URL throws DataCloneError", () => {
  // URL is not [Serializable] per the WHATWG/HTML spec, so cloning it must
  // throw a DataCloneError rather than silently producing an empty object.
  assertThrows(
    () => structuredClone(new URL("https://example.com/")),
    DOMException,
    "Cannot clone object of unsupported type.",
  );
  assertThrows(
    () => structuredClone(new URLSearchParams("a=1&b=2")),
    DOMException,
    "Cannot clone object of unsupported type.",
  );
});

Deno.test("structuredClone geometry interfaces", () => {
  const values = [
    new DOMPointReadOnly(1, 2, 3, 4),
    new DOMPoint(1, 2, 3, 4),
    new DOMRectReadOnly(1, 2, 3, 4),
    new DOMRect(1, 2, 3, 4),
    new DOMQuad(
      new DOMPoint(1, 2),
      new DOMPoint(3, 4),
      new DOMPoint(5, 6),
      new DOMPoint(7, 8),
    ),
    new DOMMatrixReadOnly([1, 2, 3, 4, 5, 6]),
    new DOMMatrix([1, 2, 3, 4, 5, 6]),
    new DOMMatrixReadOnly([
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
    ]),
  ];

  for (const value of values) {
    const clone = structuredClone(value);
    assert(clone !== value);
    assertEquals(Object.getPrototypeOf(clone), Object.getPrototypeOf(value));
    assertEquals(clone.toJSON(), value.toJSON());
    const nested = structuredClone({
      deep: [value],
      map: new Map([["value", value]]),
    });
    assertEquals(
      Object.getPrototypeOf(nested.deep[0]),
      Object.getPrototypeOf(value),
    );
    assertEquals(nested.map.get("value")?.toJSON(), value.toJSON());
    assertThrows(
      () =>
        structuredClone(value, {
          transfer: [value as unknown as Transferable],
        }),
      DOMException,
      "Value not transferable",
    );
  }

  const point = new DOMPoint(1, 2, 3, 4);
  Object.defineProperty(point, "toJSON", {
    value: () => ({ x: 10, y: 20, z: 30, w: 40 }),
  });
  const pointClone = structuredClone(point);
  assertEquals([pointClone.x, pointClone.y, pointClone.z, pointClone.w], [
    1,
    2,
    3,
    4,
  ]);

  const unrestrictedPoint = structuredClone(
    new DOMPoint(Number.NaN, Infinity, -Infinity, Number.NaN),
  );
  assert(Number.isNaN(unrestrictedPoint.x));
  assertEquals(unrestrictedPoint.y, Infinity);
  assertEquals(unrestrictedPoint.z, -Infinity);
  assert(Number.isNaN(unrestrictedPoint.w));
});

Deno.test("structuredClone ImageData and ImageBitmap", async () => {
  const data = new ImageData(new Uint8ClampedArray([1, 2, 3, 255]), 1, 1);
  const clonedData = structuredClone(data);
  assert(clonedData instanceof ImageData);
  assertEquals(clonedData.width, 1);
  assertEquals(clonedData.height, 1);
  assertEquals([...clonedData.data], [1, 2, 3, 255]);
  clonedData.data[0] = 9;
  assertEquals(data.data[0], 1);
  assertThrows(
    () =>
      structuredClone(data, { transfer: [data as unknown as Transferable] }),
    DOMException,
    "Value not transferable",
  );

  const floatData = new ImageData(
    new Float16Array([0.5, 0.25, 0.125, 1]),
    1,
    1,
    { colorSpace: "display-p3", pixelFormat: "rgba-float16" },
  );
  const clonedFloatData = structuredClone(floatData);
  assertEquals(clonedFloatData.colorSpace, "display-p3");
  assertEquals(clonedFloatData.pixelFormat, "rgba-float16");
  assertEquals([...clonedFloatData.data], [...floatData.data]);

  const bitmap = await createImageBitmap(data);
  const clonedBitmap = structuredClone(bitmap);
  assert(clonedBitmap instanceof ImageBitmap);
  assertEquals(clonedBitmap.width, bitmap.width);
  assertEquals(clonedBitmap.height, bitmap.height);
  assertEquals([...imageBitmapData(clonedBitmap)], [1, 2, 3, 255]);
  const nestedBitmap = structuredClone({ bitmap, items: [bitmap] });
  assert(nestedBitmap.bitmap !== bitmap);
  assert(nestedBitmap.bitmap === nestedBitmap.items[0]);
  assertEquals([...imageBitmapData(nestedBitmap.bitmap)], [1, 2, 3, 255]);

  assertThrows(
    () => structuredClone(bitmap, { transfer: [bitmap, bitmap] }),
    DOMException,
    "Transfer list contains duplicate item",
  );
  assertEquals(bitmap.width, 1);

  const transferredBitmap = structuredClone(bitmap, { transfer: [bitmap] });
  assert(transferredBitmap instanceof ImageBitmap);
  assertEquals(transferredBitmap.width, 1);
  assertEquals([...imageBitmapData(transferredBitmap)], [1, 2, 3, 255]);
  assertEquals(bitmap.width, 0);
  bitmap.close();
  clonedBitmap.close();
  transferredBitmap.close();
});

Deno.test("structuredClone transfers OffscreenCanvas", async () => {
  const canvas = new OffscreenCanvas(1, 1);
  const sourceContext = canvas.getContext("bitmaprenderer");
  assert(sourceContext);
  sourceContext.transferFromImageBitmap(
    await createImageBitmap(
      new ImageData(new Uint8ClampedArray([1, 2, 3, 255]), 1, 1),
    ),
  );
  assertThrows(() => structuredClone(canvas), DOMException);
  assertThrows(
    () => structuredClone(canvas, { transfer: [canvas, canvas] }),
    DOMException,
    "Transfer list contains duplicate item",
  );
  assertEquals(canvas.width, 1);
  const transferred = structuredClone(canvas, { transfer: [canvas] });
  assert(transferred instanceof OffscreenCanvas);
  assertEquals(transferred.width, 1);
  assertEquals(transferred.height, 1);
  assertEquals(canvas.width, 0);
  assertEquals(canvas.height, 0);
  assertThrows(
    () => canvas.getContext("bitmaprenderer"),
    DOMException,
    "OffscreenCanvas is detached",
  );
  const context = transferred.getContext("bitmaprenderer");
  assert(context);
  const bitmap = transferred.transferToImageBitmap();
  assertEquals([...imageBitmapData(bitmap)], [1, 2, 3, 255]);
  bitmap.close();

  const emptyCanvas = new OffscreenCanvas(0, 0);
  const transferredEmptyCanvas = structuredClone(emptyCanvas, {
    transfer: [emptyCanvas],
  });
  assertEquals(transferredEmptyCanvas.width, 0);
  assertEquals(transferredEmptyCanvas.height, 0);
});

Deno.test("postMessage URL throws DataCloneError", () => {
  // Posting a non-serializable value must take the same path as
  // structuredClone and throw, instead of silently delivering `{}`.
  const { port1, port2 } = new MessageChannel();
  try {
    assertThrows(
      () => port2.postMessage(new URL("https://example.org/")),
      DOMException,
      "Cannot clone object of unsupported type.",
    );
    assertThrows(
      () => port2.postMessage(new URLSearchParams("a=1")),
      DOMException,
      "Cannot clone object of unsupported type.",
    );
  } finally {
    port1.close();
    port2.close();
  }
});

Deno.test("postMessage transfers bitmap and offscreen canvas", async () => {
  const { port1, port2 } = new MessageChannel();
  try {
    const bitmap = await createImageBitmap(
      new ImageData(new Uint8ClampedArray([4, 5, 6, 255]), 1, 1),
    );
    const bitmapMessage = new Promise<ImageBitmap>((resolve) => {
      port2.onmessage = (event) => resolve(event.data);
    });
    port1.postMessage(bitmap, [bitmap]);
    assertEquals(bitmap.width, 0);
    const receivedBitmap = await bitmapMessage;
    assert(receivedBitmap instanceof ImageBitmap);
    assertEquals([...imageBitmapData(receivedBitmap)], [4, 5, 6, 255]);
    receivedBitmap.close();

    const canvas = new OffscreenCanvas(2, 3);
    const canvasMessage = new Promise<OffscreenCanvas>((resolve) => {
      port2.onmessage = (event) => resolve(event.data);
    });
    port1.postMessage(canvas, [canvas]);
    assertEquals(canvas.width, 0);
    const receivedCanvas = await canvasMessage;
    assert(receivedCanvas instanceof OffscreenCanvas);
    assertEquals(receivedCanvas.width, 2);
    assertEquals(receivedCanvas.height, 3);
  } finally {
    port1.close();
    port2.close();
  }
});

Deno.test("structuredClone CryptoKey", async () => {
  // AES key
  const aesKey = await crypto.subtle.generateKey(
    { name: "AES-GCM", length: 256 },
    true,
    ["encrypt", "decrypt"],
  );
  const aesClone = structuredClone(aesKey);
  assert(aesKey !== aesClone);
  assertEquals(aesClone.type, aesKey.type);
  assertEquals(aesClone.extractable, aesKey.extractable);
  assertEquals(aesClone.algorithm, aesKey.algorithm);
  assertEquals([...aesClone.usages], [...aesKey.usages]);

  // Verify the cloned key actually works
  const data = new TextEncoder().encode("hello");
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encrypted = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    aesClone,
    data,
  );
  const decrypted = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv },
    aesKey,
    encrypted,
  );
  assertEquals(new Uint8Array(decrypted), data);

  // Non-extractable key can be cloned
  const nonExtractable = await crypto.subtle.generateKey(
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
  const nonExtractableClone = structuredClone(nonExtractable);
  assertEquals(nonExtractableClone.extractable, false);
  assertEquals(nonExtractableClone.algorithm, nonExtractable.algorithm);

  // HMAC key
  const hmacKey = await crypto.subtle.generateKey(
    { name: "HMAC", hash: "SHA-256" },
    true,
    ["sign", "verify"],
  );
  const hmacClone = structuredClone(hmacKey);
  assertEquals(hmacClone.type, hmacKey.type);
  assertEquals(hmacClone.algorithm, hmacKey.algorithm);
  assertEquals([...hmacClone.usages], [...hmacKey.usages]);

  // EC key pair
  const ecKeyPair = await crypto.subtle.generateKey(
    { name: "ECDSA", namedCurve: "P-256" },
    true,
    ["sign", "verify"],
  ) as CryptoKeyPair;
  const ecPrivateClone = structuredClone(ecKeyPair.privateKey);
  const ecPublicClone = structuredClone(ecKeyPair.publicKey);
  assertEquals(ecPrivateClone.type, "private");
  assertEquals(ecPublicClone.type, "public");

  // Ed25519 key pair
  const edKeyPair = await crypto.subtle.generateKey(
    "Ed25519",
    true,
    ["sign", "verify"],
  ) as CryptoKeyPair;
  const edClone = structuredClone(edKeyPair.privateKey);
  assertEquals(edClone.type, "private");
  assertEquals(edClone.algorithm.name, "Ed25519");
});
