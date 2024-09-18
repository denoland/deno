// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, assertRejects } from "./test_util.ts";

// The following blob can be created by taking the following s-expr and pass
// it through wat2wasm.
//    (module
//      (func $add (param $a i32) (param $b i32) (result i32)
//        local.get $a
//        local.get $b
//        i32.add)
//      (export "add" (func $add))
//    )
// deno-fmt-ignore
const simpleWasm = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60,
  0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,
  0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20,
  0x00, 0x20, 0x01, 0x6a, 0x0b
]);

Deno.test(async function wasmInstantiateWorksWithBuffer() {
  const { module, instance } = await WebAssembly.instantiate(simpleWasm);
  assertEquals(WebAssembly.Module.exports(module), [{
    name: "add",
    kind: "function",
  }]);
  assertEquals(WebAssembly.Module.imports(module), []);
  assert(typeof instance.exports.add === "function");
  const add = instance.exports.add as (a: number, b: number) => number;
  assertEquals(add(1, 3), 4);
});

// V8's default implementation of `WebAssembly.instantiateStreaming()` if you
// don't set the WASM streaming callback, is to take a byte source. Here we
// check that our implementation of the callback disallows it.
Deno.test(
  async function wasmInstantiateStreamingFailsWithBuffer() {
    await assertRejects(async () => {
      await WebAssembly.instantiateStreaming(
        // Bypassing the type system
        simpleWasm as unknown as Promise<Response>,
      );
    }, TypeError);
  },
);

Deno.test(
  async function wasmInstantiateStreamingNoContentType() {
    const response = new Response(simpleWasm);
    // Rejects, not throws.
    const wasmPromise = WebAssembly.instantiateStreaming(response);
    await assertRejects(
      () => wasmPromise,
      TypeError,
      "Invalid WebAssembly content type",
    );
  },
);

Deno.test(async function wasmInstantiateStreaming() {
  let isomorphic = "";
  for (const byte of simpleWasm) {
    isomorphic += String.fromCharCode(byte);
  }
  const base64Url = "data:application/wasm;base64," + btoa(isomorphic);

  const { module, instance } = await WebAssembly.instantiateStreaming(
    fetch(base64Url),
  );
  assertEquals(WebAssembly.Module.exports(module), [{
    name: "add",
    kind: "function",
  }]);
  assertEquals(WebAssembly.Module.imports(module), []);
  assert(typeof instance.exports.add === "function");
  const add = instance.exports.add as (a: number, b: number) => number;
  assertEquals(add(1, 3), 4);
});

Deno.test(
  { permissions: { read: true } },
  async function wasmFileStreaming() {
    const url = import.meta.resolve("../testdata/assets/unreachable.wasm");
    assert(url.startsWith("file://"));

    const { module } = await WebAssembly.instantiateStreaming(fetch(url));
    assertEquals(WebAssembly.Module.exports(module), [{
      name: "unreachable",
      kind: "function",
    }]);
  },
);

Deno.test(
  { permissions: { net: true } },
  async function wasmStreamingNonTrivial() {
    // deno-dom's WASM file is a real-world non-trivial case that gave us
    // trouble when implementing this.
    await WebAssembly.instantiateStreaming(fetch(
      "http://localhost:4545/assets/deno_dom_0.1.3-alpha2.wasm",
    ));
  },
);
