// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows, unitTest } from "./test_util.ts";

unitTest(function fromInit(): void {
  const req = new Request("http://foo/", {
    body: "ahoyhoy",
    method: "POST",
    headers: {
      "test-header": "value",
    },
  });

  // deno-lint-ignore no-explicit-any
  assertEquals("ahoyhoy", (req as any)._bodySource);
  assertEquals(req.url, "http://foo/");
  assertEquals(req.headers.get("test-header"), "value");
});

unitTest(function fromRequest(): void {
  const r = new Request("http://foo/");
  // deno-lint-ignore no-explicit-any
  (r as any)._bodySource = "ahoyhoy";
  r.headers.set("test-header", "value");

  const req = new Request(r);

  // deno-lint-ignore no-explicit-any
  assertEquals((req as any)._bodySource, (r as any)._bodySource);
  assertEquals(req.url, r.url);
  assertEquals(req.headers.get("test-header"), r.headers.get("test-header"));
});

unitTest(function requestNonString(): void {
  const nonString = {
    toString() {
      return "http://foo/";
    },
  };
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  assertEquals(new Request(nonString).url, "http://foo/");
});

unitTest(function methodNonString(): void {
  assertEquals(new Request("http://foo/", { method: undefined }).method, "GET");
});

unitTest(function requestRelativeUrl(): void {
  // TODO(nayeemrmn): Base from `--location` when implemented and set.
  assertThrows(() => new Request("relative-url"), TypeError, "Invalid URL.");
});

unitTest(async function cloneRequestBodyStream(): Promise<void> {
  // hack to get a stream
  const stream = new Request("http://foo/", { body: "a test body" }).body;
  const r1 = new Request("http://foo/", {
    body: stream,
  });

  const r2 = r1.clone();

  const b1 = await r1.text();
  const b2 = await r2.text();

  assertEquals(b1, b2);

  // deno-lint-ignore no-explicit-any
  assert((r1 as any)._bodySource !== (r2 as any)._bodySource);
});
