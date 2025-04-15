// Copyright 2018-2025 the Deno authors. MIT license.
import { assert, assertEquals, assertRejects } from "./test_util.ts";

// just a hack to get a body object
// deno-lint-ignore no-explicit-any
function buildBody(body: any, headers?: Headers): Body {
  const stub = new Request("http://foo/", {
    body: body,
    headers,
    method: "POST",
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
Deno.test(async function arrayBufferFromByteArrays() {
  const buffer = new TextEncoder().encode("ahoyhoy8").buffer;

  for (const type of intArrays) {
    const body = buildBody(new type(buffer as ArrayBuffer));
    const text = new TextDecoder("utf-8").decode(await body.arrayBuffer());
    assertEquals(text, "ahoyhoy8");
  }
});

//FormData
Deno.test(
  { permissions: { net: true } },
  async function bodyMultipartFormData() {
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
  },
);

// FormData: non-ASCII names and filenames
Deno.test(
  { permissions: { net: true } },
  async function bodyMultipartFormDataNonAsciiNames() {
    const boundary = "----01230123";
    const payload = [
      `--${boundary}`,
      `Content-Disposition: form-data; name="文字"`,
      "",
      "文字",
      `--${boundary}`,
      `Content-Disposition: form-data; name="file"; filename="文字"`,
      "Content-Type: application/octet-stream",
      "",
      "",
      `--${boundary}--`,
    ].join("\r\n");

    const body = buildBody(
      new TextEncoder().encode(payload),
      new Headers({
        "Content-Type": `multipart/form-data; boundary=${boundary}`,
      }),
    );

    const formData = await body.formData();
    assert(formData.has("文字"));
    assertEquals(formData.get("文字"), "文字");
    assert(formData.has("file"));
    assert(formData.get("file") instanceof File);
    assertEquals((formData.get("file") as File).name, "文字");
  },
);

// FormData: non-ASCII names and filenames roundtrip
Deno.test(
  { permissions: { net: true } },
  async function bodyMultipartFormDataNonAsciiRoundtrip() {
    const inFormData = new FormData();
    inFormData.append("文字", "文字");
    inFormData.append("file", new File([], "文字"));

    const body = buildBody(inFormData);

    const formData = await body.formData();
    assert(formData.has("文字"));
    assertEquals(formData.get("文字"), "文字");
    assert(formData.has("file"));
    assert(formData.get("file") instanceof File);
    assertEquals((formData.get("file") as File).name, "文字");
  },
);

Deno.test(
  { permissions: { net: true } },
  async function bodyURLEncodedFormData() {
    const response = await fetch(
      "http://localhost:4545/subdir/form_urlencoded.txt",
    );
    assert(response.body instanceof ReadableStream);

    const text = await response.text();

    const body = buildBody(text, response.headers);

    const formData = await body.formData();
    assert(formData.has("field_1"));
    assertEquals(formData.get("field_1")!.toString(), "Hi");
    assert(formData.has("field_2"));
    assertEquals(formData.get("field_2")!.toString(), "<Deno>");
  },
);

Deno.test({ permissions: {} }, async function bodyURLSearchParams() {
  const body = buildBody(new URLSearchParams({ hello: "world" }));

  const text = await body.text();
  assertEquals(text, "hello=world");
});

Deno.test(async function bodyArrayBufferMultipleParts() {
  const parts: Uint8Array[] = [];
  let size = 0;
  for (let i = 0; i <= 150000; i++) {
    const part = new Uint8Array([1]);
    parts.push(part);
    size += part.length;
  }

  let offset = 0;
  const stream = new ReadableStream({
    pull(controller) {
      // parts.shift() takes forever: https://github.com/denoland/deno/issues/5259
      const chunk = parts[offset++];
      if (!chunk) return controller.close();
      controller.enqueue(chunk);
    },
  });

  const body = buildBody(stream);
  assertEquals((await body.arrayBuffer()).byteLength, size);
});

// https://github.com/denoland/deno/issues/20793
Deno.test(
  { permissions: { net: true } },
  async function bodyMultipartFormDataMultipleHeaders() {
    const boundary = "----formdata-polyfill-0.970665446687947";
    const payload = [
      "------formdata-polyfill-0.970665446687947",
      'Content-Disposition: form-data; name="x"; filename="blob"',
      "Content-Length: 1",
      "Content-Type: application/octet-stream",
      "last-modified: Wed, 04 Oct 2023 20:28:45 GMT",
      "",
      "y",
      "------formdata-polyfill-0.970665446687947--",
    ].join("\r\n");

    const body = buildBody(
      new TextEncoder().encode(payload),
      new Headers({
        "Content-Type": `multipart/form-data; boundary=${boundary}`,
      }),
    );

    const formData = await body.formData();
    const file = formData.get("x");
    assert(file instanceof File);
    const text = await file.text();
    assertEquals(text, "y");
    assertEquals(file.size, 1);
  },
);

Deno.test(async function bodyBadResourceError() {
  const file = await Deno.open("README.md");
  file.close();
  const body = buildBody(file.readable);
  await assertRejects(
    () => body.arrayBuffer(),
    Deno.errors.BadResource,
    "Cannot read body as underlying resource unavailable",
  );
});
