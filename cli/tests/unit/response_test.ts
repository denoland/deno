// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test(async function responseText() {
  const response = new Response("hello world");
  const textPromise = response.text();
  assert(textPromise instanceof Promise);
  const text = await textPromise;
  assert(typeof text === "string");
  assertEquals(text, "hello world");
});

Deno.test(async function responseArrayBuffer() {
  const response = new Response(new Uint8Array([1, 2, 3]));
  const arrayBufferPromise = response.arrayBuffer();
  assert(arrayBufferPromise instanceof Promise);
  const arrayBuffer = await arrayBufferPromise;
  assert(arrayBuffer instanceof ArrayBuffer);
  assertEquals(new Uint8Array(arrayBuffer), new Uint8Array([1, 2, 3]));
});

Deno.test(async function responseJson() {
  const response = new Response('{"hello": "world"}');
  const jsonPromise = response.json();
  assert(jsonPromise instanceof Promise);
  const json = await jsonPromise;
  assert(json instanceof Object);
  assertEquals(json, { hello: "world" });
});

Deno.test(async function responseBlob() {
  const response = new Response(new Uint8Array([1, 2, 3]));
  const blobPromise = response.blob();
  assert(blobPromise instanceof Promise);
  const blob = await blobPromise;
  assert(blob instanceof Blob);
  assertEquals(blob.size, 3);
  assertEquals(await blob.arrayBuffer(), new Uint8Array([1, 2, 3]).buffer);
});

Deno.test(async function responseFormData() {
  const input = new FormData();
  input.append("hello", "world");
  const response = new Response(input);
  const contentType = response.headers.get("content-type")!;
  assert(contentType.startsWith("multipart/form-data"));
  const formDataPromise = response.formData();
  assert(formDataPromise instanceof Promise);
  const formData = await formDataPromise;
  assert(formData instanceof FormData);
  assertEquals([...formData], [...input]);
});

Deno.test(function responseInvalidInit() {
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  assertThrows(() => new Response("", 0));
  assertThrows(() => new Response("", { status: 0 }));
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  assertThrows(() => new Response("", { status: null }));
});

Deno.test(function responseNullInit() {
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  const response = new Response("", null);
  assertEquals(response.status, 200);
});

Deno.test(function customInspectFunction() {
  const response = new Response();
  assertEquals(
    Deno.inspect(response),
    `Response {
  body: null,
  bodyUsed: false,
  headers: Headers {},
  ok: true,
  redirected: false,
  status: 200,
  statusText: "",
  url: ""
}`,
  );
  assertStringIncludes(Deno.inspect(Response.prototype), "Response");
});
