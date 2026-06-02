// Copyright 2018-2026 the Deno authors. MIT license.
import {
  assertEquals,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test(async function fromInit() {
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

Deno.test(function requestNonString() {
  const nonString = {
    toString() {
      return "http://foo/";
    },
  };
  // deno-lint-ignore ban-ts-comment
  // @ts-expect-error
  assertEquals(new Request(nonString).url, "http://foo/");
});

Deno.test(function methodNonString() {
  assertEquals(new Request("http://foo/", { method: undefined }).method, "GET");
});

Deno.test(function requestRelativeUrl() {
  assertEquals(
    new Request("relative-url").url,
    "http://127.0.0.1:4545/relative-url",
  );
});

Deno.test(async function cloneRequestBodyStream() {
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

Deno.test(function customInspectFunction() {
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

Deno.test(function requestConstructorTakeURLObjectAsParameter() {
  assertEquals(
    new Request(new URL("http://foo/")).url,
    "http://foo/",
  );
});

Deno.test(function requestDefaultProperties() {
  const req = new Request("http://foo/");
  assertEquals(req.cache, "default");
  assertEquals(req.credentials, "same-origin");
  assertEquals(req.integrity, "");
  assertEquals(req.keepalive, false);
  assertEquals(req.mode, "cors");
  assertEquals(req.priority, "auto");
  assertEquals(req.referrer, "about:client");
  assertEquals(req.referrerPolicy, "");
});

Deno.test(function requestInitProperties() {
  const req = new Request("http://foo/", {
    cache: "no-store",
    credentials: "include",
    integrity: "sha256-abc",
    keepalive: true,
    mode: "no-cors",
    priority: "high",
    referrer: "http://example.com/",
    referrerPolicy: "no-referrer",
  });
  assertEquals(req.cache, "no-store");
  assertEquals(req.credentials, "include");
  assertEquals(req.integrity, "sha256-abc");
  assertEquals(req.keepalive, true);
  assertEquals(req.mode, "no-cors");
  assertEquals(req.priority, "high");
  assertEquals(req.referrer, "http://example.com/");
  assertEquals(req.referrerPolicy, "no-referrer");
});

Deno.test(function requestInvalidPriorityThrows() {
  assertThrows(
    () => new Request("http://foo/", { priority: "bogus" as RequestPriority }),
    TypeError,
  );
});

Deno.test(function requestReferrerEmptyString() {
  const req = new Request("http://foo/", { referrer: "" });
  assertEquals(req.referrer, "");
});

Deno.test(function requestReferrerAboutClient() {
  const req = new Request("http://foo/", { referrer: "about:client" });
  assertEquals(req.referrer, "about:client");
});

Deno.test(function requestModeNavigateThrows() {
  assertThrows(
    () => new Request("http://foo/", { mode: "navigate" }),
    TypeError,
  );
});

Deno.test(function requestOnlyIfCachedRequiresSameOrigin() {
  assertThrows(
    () => new Request("http://foo/", { cache: "only-if-cached" }),
    TypeError,
  );
  // Allowed when mode is same-origin.
  const req = new Request("http://foo/", {
    cache: "only-if-cached",
    mode: "same-origin",
  });
  assertEquals(req.cache, "only-if-cached");
  assertEquals(req.mode, "same-origin");
});

Deno.test(function requestPropertiesInheritedFromRequest() {
  const original = new Request("http://foo/", {
    cache: "no-store",
    credentials: "include",
    integrity: "sha256-abc",
    keepalive: true,
    mode: "no-cors",
    priority: "low",
    referrer: "http://example.com/",
    referrerPolicy: "origin",
  });
  const cloned = new Request(original);
  assertEquals(cloned.cache, "no-store");
  assertEquals(cloned.credentials, "include");
  assertEquals(cloned.integrity, "sha256-abc");
  assertEquals(cloned.keepalive, true);
  assertEquals(cloned.mode, "no-cors");
  assertEquals(cloned.priority, "low");
  assertEquals(cloned.referrer, "http://example.com/");
  assertEquals(cloned.referrerPolicy, "origin");
});

Deno.test(function requestCloneCopiesAllProperties() {
  const original = new Request("http://foo/", {
    cache: "force-cache",
    credentials: "omit",
    integrity: "sha384-xyz",
    keepalive: true,
    mode: "cors",
    priority: "high",
    referrer: "http://example.com/",
    referrerPolicy: "strict-origin",
  });
  const cloned = original.clone();
  assertEquals(cloned.cache, "force-cache");
  assertEquals(cloned.credentials, "omit");
  assertEquals(cloned.integrity, "sha384-xyz");
  assertEquals(cloned.keepalive, true);
  assertEquals(cloned.mode, "cors");
  assertEquals(cloned.priority, "high");
  assertEquals(cloned.referrer, "http://example.com/");
  assertEquals(cloned.referrerPolicy, "strict-origin");
});
