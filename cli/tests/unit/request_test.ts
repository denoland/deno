// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertStringIncludes, unitTest } from "./test_util.ts";

unitTest(async function fromInit() {
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

unitTest(function requestNonString() {
  const nonString = {
    toString() {
      return "http://foo/";
    },
  };
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  assertEquals(new Request(nonString).url, "http://foo/");
});

unitTest(function methodNonString() {
  assertEquals(new Request("http://foo/", { method: undefined }).method, "GET");
});

unitTest(function requestRelativeUrl() {
  assertEquals(
    new Request("relative-url").url,
    "http://js-unit-tests/foo/relative-url",
  );
});

unitTest(async function cloneRequestBodyStream() {
  // hack to get a stream
  const stream =
    new Request("http://foo/", { body: "a test body", method: "POST" }).body;
  const r1 = new Request("http://foo/", {
    body: stream,
    method: "POST",
  });

  const r2 = r1.clone();

  const b1 = await r1.text();
  const b2 = await r2.text();

  assertEquals(b1, b2);
});

unitTest(function customInspectFunction() {
  const request = new Request("https://example.com");
  assertEquals(
    Deno.inspect(request),
    `Request {
  bodyUsed: false,
  headers: Headers {},
  method: "GET",
  redirect: "follow",
  url: "https://example.com/"
}`,
  );
  assertStringIncludes(Deno.inspect(Request.prototype), "Request");
});
