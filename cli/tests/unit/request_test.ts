// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";

unitTest(async function fromInit(): Promise<void> {
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

unitTest(async function fromRequest(): Promise<void> {
  const r = new Request("http://foo/", { body: "ahoyhoy" });
  r.headers.set("test-header", "value");

  const req = new Request(r);

  assertEquals(await r.text(), await req.text());
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
  assertEquals(
    new Request("relative-url").url,
    "http://js-unit-tests/foo/relative-url",
  );
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
});

unitTest(function requiresOneArgument() {
  assertThrows(() => {
    // @ts-expect-error construct signature arity mismatch
    new Request();
  });
});

unitTest(function acceptsStringObjects() {
  // note: similar to `requestNonString`, yet `String` objects broke past JS, reaching Rust, causing errors
  const url = "http://foo/";
  {
    // @ts-expect-error construct signature type mismatch
    const request = new Request(new String(url));
    assertEquals(request.url, url);
  }

  {
    const objectURL = new URL(url);
    // @ts-expect-error construct signature type mismatch
    const request = new Request(objectURL);
    assertEquals(request.url, url);
  }
});

unitTest(function castsInitializerToDictionary(): void {
  const url = "https://foo/";

  type initializerPasser = (_: unknown) => void;

  const acceptsInitializer: initializerPasser = (requestInit) => {
    // @ts-expect-error construct signature mismatch
    const request = new Request(url, requestInit);

    assertEquals(request.url, url);
    // TODO(#9498) add more asserts to generally make sure nothing weird went wrong
  };

  const allowedInitializers: unknown[] = [
    {},
    [],
    () => {},
    null,
    undefined,
  ];

  allowedInitializers.map(acceptsInitializer);

  const deniesInitializer: initializerPasser = (requestInit) => {
    assertThrows(
      () => {
        // @ts-expect-error construct signature type mismatch
        new Request(url, requestInit);
      },
    );
  };

  const disallowedInitializers = [
    0,
    0n,
    "",
    false,
    Symbol(),
  ];

  disallowedInitializers.map(deniesInitializer);
});

unitTest(function acceptsRequestObjects() {
  const requestObject = new Request("http://foo/");
  const copiedRequest = new Request(requestObject);

  assert(requestObject !== copiedRequest);
  assertEquals(requestObject.url, copiedRequest.url);
  assertEquals(requestObject.method, copiedRequest.method);
  assertEquals(requestObject.bodyUsed, copiedRequest.bodyUsed);
  assertEquals(requestObject.redirect, copiedRequest.redirect);
});
