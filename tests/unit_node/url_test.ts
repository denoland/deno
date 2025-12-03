// Copyright 2018-2025 the Deno authors. MIT license.
import { domainToASCII } from "node:url";
import { assertEquals } from "@std/assert/equals";

Deno.test({
  name: "[node/url] domainToASCII",
  fn() {
    assertEquals(domainToASCII("example.com"), "example.com");
    assertEquals(domainToASCII("[::1]"), "[::1]");
    assertEquals(domainToASCII("münchen.de"), "xn--mnchen-3ya.de");
    // Invalid domain returns empty string
    assertEquals(domainToASCII("xn--iñvalid.com"), "");
  },
});
