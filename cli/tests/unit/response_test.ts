// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "./test_util.ts";

Deno.test("responseText", async function () {
  const response = new Response("hello world");
  const textPromise = response.text();
  assert(textPromise instanceof Promise);
  const text = await textPromise;
  assert(typeof text === "string");
  assertEquals(text, "hello world");
});

Deno.test("responseArrayBuffer", async function () {
  const response = new Response(new Uint8Array([1, 2, 3]));
  const arrayBufferPromise = response.arrayBuffer();
  assert(arrayBufferPromise instanceof Promise);
  const arrayBuffer = await arrayBufferPromise;
  assert(arrayBuffer instanceof ArrayBuffer);
  assertEquals(new Uint8Array(arrayBuffer), new Uint8Array([1, 2, 3]));
});

Deno.test("responseJson", async function () {
  const response = new Response('{"hello": "world"}');
  const jsonPromise = response.json();
  assert(jsonPromise instanceof Promise);
  const json = await jsonPromise;
  assert(json instanceof Object);
  assertEquals(json, { hello: "world" });
});

Deno.test("responseBlob", async function () {
  const response = new Response(new Uint8Array([1, 2, 3]));
  const blobPromise = response.blob();
  assert(blobPromise instanceof Promise);
  const blob = await blobPromise;
  assert(blob instanceof Blob);
  assertEquals(blob, new Blob([new Uint8Array([1, 2, 3])]));
});

Deno.test({
  name: "responseFormData",
  // TODO(lucacasonato): re-enable test once #10002 is fixed.
  ignore: true,
  async fn() {
    const input = new FormData();
    input.append("hello", "world");
    const response = new Response(input, {
      headers: { "content-type": "application/x-www-form-urlencoded" },
    });
    const formDataPromise = response.formData();
    assert(formDataPromise instanceof Promise);
    const formData = await formDataPromise;
    assert(formData instanceof FormData);
    assertEquals(formData, input);
  },
});
