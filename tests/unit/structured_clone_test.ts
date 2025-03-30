// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, assertEquals, assertThrows } from "./test_util.ts";

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
