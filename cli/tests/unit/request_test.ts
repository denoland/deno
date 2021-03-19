// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test("fromInit", async function (): Promise<void> {
  const req = new Request("http://foo/", {
    body: "ahoyhoy",
    method: "POST",
    headers: {
      "test-header": "value",
    },
  });

  assertEquals("ahoyhoy", await req.text());
  assertEquals(req.url, "http://foo/");
  assertEquals(req.headers.get("test-header"), "value");
});

Deno.test("fromRequest", async function (): Promise<void> {
  const r = new Request("http://foo/", { body: "ahoyhoy" });
  r.headers.set("test-header", "value");

  const req = new Request(r);

  assertEquals(await r.text(), await req.text());
  assertEquals(req.url, r.url);
  assertEquals(req.headers.get("test-header"), r.headers.get("test-header"));
});

Deno.test("requestNonString", function (): void {
  const nonString = {
    toString() {
      return "http://foo/";
    },
  };
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  assertEquals(new Request(nonString).url, "http://foo/");
});

Deno.test("methodNonString", function (): void {
  assertEquals(new Request("http://foo/", { method: undefined }).method, "GET");
});

Deno.test("requestRelativeUrl", function (): void {
  assertEquals(
    new Request("relative-url").url,
    "http://js-unit-tests/foo/relative-url",
  );
});

Deno.test("cloneRequestBodyStream", async function (): Promise<void> {
  // hack to get a stream
  const stream = new Request("http://foo/", { body: "a test body" }).body;
  const r1 = new Request("http://foo/", {
    body: stream,
  });

  const r2 = r1.clone();

  const b1 = await r1.text();
  const b2 = await r2.text();

  assertEquals(b1, b2);
});
