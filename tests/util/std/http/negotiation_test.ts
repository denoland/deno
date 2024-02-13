// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { accepts, acceptsEncodings, acceptsLanguages } from "./negotiation.ts";

Deno.test({
  name: "accepts - no args",
  fn() {
    const req = new Request("https://example.com/", {
      headers: {
        "accept":
          "text/html, application/xhtml+xml, application/xml;q=0.9, image/webp, */*;q=0.8",
      },
    });
    assertEquals(accepts(req), [
      "text/html",
      "application/xhtml+xml",
      "image/webp",
      "application/xml",
      "*/*",
    ]);
  },
});

Deno.test({
  name: "accepts - args",
  fn() {
    const req = new Request("https://example.com/", {
      headers: {
        "accept":
          "text/html, application/xhtml+xml, application/xml;q=0.9, image/webp, */*;q=0.8",
      },
    });
    assertEquals(accepts(req, "text/html", "image/webp"), "text/html");
  },
});

Deno.test({
  name: "accepts - no match",
  fn() {
    const req = new Request("https://example.com/", {
      headers: {
        "accept": "text/html, application/xhtml+xml, application/xml",
      },
    });
    assertEquals(accepts(req, "application/json"), undefined);
  },
});

Deno.test({
  name: "accepts - args + no header",
  fn() {
    const req = new Request("https://example.com/");
    assertEquals(accepts(req, "text/html", "image/webp"), "text/html");
  },
});

Deno.test({
  name: "accepts - no args + no header",
  fn() {
    const req = new Request("https://example.com/");
    assertEquals(accepts(req), ["*/*"]);
  },
});

Deno.test({
  name: "acceptsEncodings - no args",
  fn() {
    const req = new Request("https://example.com/", {
      headers: { "accept-encoding": "deflate, gzip;q=1.0, *;q=0.5" },
    });
    assertEquals(acceptsEncodings(req), ["deflate", "gzip", "*"]);
  },
});

Deno.test({
  name: "acceptsEncodings - args",
  fn() {
    const req = new Request("https://example.com/", {
      headers: { "accept-encoding": "deflate, gzip;q=1.0, *;q=0.5" },
    });
    assertEquals(acceptsEncodings(req, "gzip", "identity"), "gzip");
  },
});

Deno.test({
  name: "acceptsEncodings - no match",
  fn() {
    const req = new Request("https://example.com/", {
      headers: { "accept-encoding": "deflate, gzip" },
    });
    assertEquals(acceptsEncodings(req, "brotli"), undefined);
  },
});

Deno.test({
  name: "acceptsEncodings - args + no header",
  fn() {
    const req = new Request("https://example.com/");
    assertEquals(acceptsEncodings(req, "gzip", "identity"), "gzip");
  },
});

Deno.test({
  name: "acceptsEncodings - no args + no header",
  fn() {
    const req = new Request("https://example.com/");
    assertEquals(acceptsEncodings(req), ["*"]);
  },
});

Deno.test({
  name: "acceptsLanguages - no args",
  fn() {
    const req = new Request("https://example.com/", {
      headers: {
        "accept-language": "fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5",
      },
    });
    assertEquals(acceptsLanguages(req), ["fr-CH", "fr", "en", "de", "*"]);
  },
});

Deno.test({
  name: "acceptsLanguages - args",
  fn() {
    const req = new Request("https://example.com/", {
      headers: {
        "accept-language": "fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5",
      },
    });
    assertEquals(acceptsLanguages(req, "en-gb", "en-us", "en"), "en");
  },
});

Deno.test({
  name: "acceptsLanguages - no match",
  fn() {
    const req = new Request("https://example.com/", {
      headers: { "accept-language": "fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7" },
    });
    assertEquals(acceptsLanguages(req, "zh"), undefined);
  },
});

Deno.test({
  name: "acceptsLanguages - args + no header",
  fn() {
    const req = new Request("https://example.com/");
    assertEquals(acceptsLanguages(req, "en-gb", "en-us", "en"), "en-gb");
  },
});

Deno.test({
  name: "acceptsLanguages - no args + no header",
  fn() {
    const req = new Request("https://example.com/");
    assertEquals(acceptsLanguages(req), ["*"]);
  },
});
