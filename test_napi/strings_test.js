// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertRejects, loadTestLibrary } from "./common.js";

const strings = loadTestLibrary();

const simpleWasm = new Uint8Array([
  0x00,
  0x61,
  0x73,
  0x6d,
  0x01,
  0x00,
  0x00,
  0x00,
  0x01,
  0x07,
  0x01,
  0x60,
  0x02,
  0x7f,
  0x7f,
  0x01,
  0x7f,
  0x03,
  0x02,
  0x01,
  0x00,
  0x07,
  0x07,
  0x01,
  0x03,
  0x61,
  0x64,
  0x64,
  0x00,
  0x00,
  0x0a,
  0x09,
  0x01,
  0x07,
  0x00,
  0x20,
  0x00,
  0x20,
  0x01,
  0x6a,
  0x0b,
]);

Deno.test("napi string utf8", function () {
  assertEquals(strings.test_utf8(""), "");
  assertEquals(strings.test_utf8("🦕"), "🦕");
});

Deno.test("napi string", function () {
  assertEquals(strings.test_utf16(""), "");
  assertEquals(strings.test_utf16("🦕"), "🦕");
});

Deno.test(
  async function wasmInstantiateStreamingNoContentType() {
    await assertRejects(
      async () => {
        const response = Promise.resolve(new Response(simpleWasm));
        await WebAssembly.instantiateStreaming(response);
      },
      TypeError,
      "Invalid WebAssembly content type.",
    );
  },
);
