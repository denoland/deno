// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "./test_util.ts";

// just a hack to get a body object
// deno-lint-ignore no-explicit-any
function buildBody(body: any, headers?: Headers): Body {
  const stub = new Request("http://foo/", {
    body: body,
    headers,
  });
  return stub as Body;
}

const intArrays = [
  Int8Array,
  Int16Array,
  Int32Array,
  Uint8Array,
  Uint16Array,
  Uint32Array,
  Uint8ClampedArray,
  Float32Array,
  Float64Array,
];

Deno.test("arrayBufferFromByteArrays", async function (): Promise<void> {
  const buffer = new TextEncoder().encode("ahoyhoy8").buffer;

  for (const type of intArrays) {
    const body = buildBody(new type(buffer));
    const text = new TextDecoder("utf-8").decode(await body.arrayBuffer());
    assertEquals(text, "ahoyhoy8");
  }
});

//FormData
Deno.test("bodyMultipartFormData", async function (): Promise<void> {
  const response = await fetch(
    "http://localhost:4545/multipart_form_data.txt",
  );
  assert(response.body instanceof ReadableStream);

  const text = await response.text();

  const body = buildBody(text, response.headers);

  const formData = await body.formData();
  assert(formData.has("field_1"));
  assertEquals(formData.get("field_1")!.toString(), "value_1 \r\n");
  assert(formData.has("field_2"));
});

Deno.test("bodyURLEncodedFormData", async function (): Promise<void> {
  const response = await fetch(
    "http://localhost:4545/cli/tests/subdir/form_urlencoded.txt",
  );
  assert(response.body instanceof ReadableStream);

  const text = await response.text();

  const body = buildBody(text, response.headers);

  const formData = await body.formData();
  assert(formData.has("field_1"));
  assertEquals(formData.get("field_1")!.toString(), "Hi");
  assert(formData.has("field_2"));
  assertEquals(formData.get("field_2")!.toString(), "<Deno>");
});

Deno.test("bodyURLSearchParams", async function (): Promise<void> {
  const body = buildBody(new URLSearchParams({ hello: "world" }));

  const text = await body.text();
  assertEquals(text, "hello=world");
});

Deno.test("bodyArrayBufferMultipleParts", async function (): Promise<void> {
  const parts: Uint8Array[] = [];
  let size = 0;
  for (let i = 0; i <= 150000; i++) {
    const part = new Uint8Array([1]);
    parts.push(part);
    size += part.length;
  }

  let offset = 0;
  const stream = new ReadableStream({
    pull(controller): void {
      // parts.shift() takes forever: https://github.com/denoland/deno/issues/5259
      const chunk = parts[offset++];
      if (!chunk) return controller.close();
      controller.enqueue(chunk);
    },
  });

  const body = buildBody(stream);
  assertEquals((await body.arrayBuffer()).byteLength, size);
});
