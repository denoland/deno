// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, unitTest } from "./test_util.ts";

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
    // deno-lint-ignore ban-ts-comment
    // @ts-expect-error
    new Request();
  });
});

unitTest(function acceptsStringObjects() {
  const url = "http://foo/";
  {
    const request = new Request(new String(url));
    assertEquals(request.url, url);
  }

  {
    const objectURL = new URL(url, location.href);
    const request = new Request(objectURL);
    assertEquals(request.url, url);
  }
});

unitTest(function castsInitializerToDictionary(): void {
  const url = "https://foo/";

  type initializerPasser = (_: unknown) => void;

  const acceptsInitializer: initializerPasser = (requestInit) => {
    // deno-lint-ignore ban-ts-comment
    // @ts-expect-error
    const request = new Request(url, requestInit);

    assertEquals(request.url, url);
    // add more asserts to generally make sure nothing weird went wrong
  };

  acceptsInitializer({});
  acceptsInitializer([]);
  acceptsInitializer(() => {});
  acceptsInitializer(null);
  acceptsInitializer(undefined);

  const deniesInitializer: initializerPasser = (requestInit) => {
    assertThrows(
      () => {
        // deno-lint-ignore ban-ts-comment
        // @ts-expect-error
        new Request(url, requestInit);
      },
    );
  };

  deniesInitializer(0);
  deniesInitializer(0n);
  deniesInitializer("");
  deniesInitializer(false);
  deniesInitializer(Symbol());
});
