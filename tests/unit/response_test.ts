// Copyright 2018-2026 the Deno authors. MIT license.
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

Deno.test(async function responseBodyUsed() {
  const response = new Response("body");
  assert(!response.bodyUsed);
  await response.text();
  assert(response.bodyUsed);
  // .body getter is needed so we can test the faulty code path
  response.body;
  assert(response.bodyUsed);
});

// A body extracted from a BodyInit (string, typed array, ...) must be exposed
// as a readable byte stream, so a BYOB reader can be acquired from it.
// https://github.com/denoland/deno/issues/17386
Deno.test(async function responseBodyByobReader() {
  async function readByob(stream: ReadableStream<Uint8Array>) {
    const reader = stream.getReader({ mode: "byob" });
    const chunks: Uint8Array[] = [];
    let total = 0;
    while (true) {
      const { value, done } = await reader.read(new Uint8Array(16));
      if (done) break;
      chunks.push(value);
      total += value.byteLength;
    }
    const result = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
      result.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return result;
  }

  assertEquals(
    await readByob(new Response(new Uint8Array([1, 2, 3])).body!),
    new Uint8Array([1, 2, 3]),
  );
  assertEquals(
    await readByob(new Response("foo").body!),
    new TextEncoder().encode("foo"),
  );
  assertEquals(
    await readByob(new Response(new Uint8Array()).body!),
    new Uint8Array(),
  );
  assertEquals(
    await readByob(
      new Request("http://localhost/", { method: "POST", body: "bar" }).body!,
    ),
    new TextEncoder().encode("bar"),
  );

  // A cloned body shares the static source with the original. Reading one as a
  // byte stream must not detach the buffer out from under the other.
  const original = new Response(new Uint8Array([4, 5, 6]));
  const cloned = original.clone();
  assertEquals(await readByob(original.body!), new Uint8Array([4, 5, 6]));
  assertEquals(await readByob(cloned.body!), new Uint8Array([4, 5, 6]));
});
