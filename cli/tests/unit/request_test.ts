// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
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
    "http://js-unit-tests/foo/relative-url",
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
  url: "https://example.com/",
  referrer: "about:client",
  referrerPolicy: ""
}`,
  );
  assertStringIncludes(Deno.inspect(Request.prototype), "Request");
});

Deno.test(function requestReferrerEmpty() {
  assertEquals(
    new Request("https://google.com", { referrer: "" }).referrer,
    "",
  );
});

Deno.test(function requestReferrerAboutClient() {
  assertEquals(
    new Request("https://google.com").referrer,
    "about:client",
  );
});

Deno.test(function requestReferrerCustom() {
  assertEquals(
    new Request("https://google.com", { referrer: "https://google.com" })
      .referrer,
    "https://google.com/",
  );
});

Deno.test(function requestReferrerTypeError() {
  assertThrows(
    () => {
      new Request("https://google.com", { referrer: "aaa" });
    },
    TypeError,
  );
});

Deno.test(function requestReferrerPolicy() {
  [
    "",
    "no-referrer",
    "no-referrer-when-downgrade",
    "origin",
    "origin-when-cross-origin",
    "same-origin",
    "strict-origin",
    "strict-origin-when-cross-origin",
    "unsafe-url",
  ].forEach((rp: string) => {
    assertEquals(
      new Request("https://google.com", {
        referrerPolicy: rp as ReferrerPolicy,
      }).referrerPolicy,
      rp,
    );
  });
});

Deno.test(function requestReferrerPolicyTypeError() {
  assertThrows(
    () => {
      // @ts-ignore Testing invalid referrerPolicy
      new Request("https://google.com", { referrerPolicy: "aaa" });
    },
    TypeError,
  );
});

Deno.test(function requestConstructorTakeURLObjectAsParameter() {
  assertEquals(
    new Request(new URL("http://foo/")).url,
    "http://foo/",
  );
});
