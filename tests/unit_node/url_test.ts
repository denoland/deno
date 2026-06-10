// Copyright 2018-2026 the Deno authors. MIT license.
import { domainToASCII, format } from "node:url";
import { assertEquals, assertThrows } from "@std/assert";

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

Deno.test({
  name: "[node/url] format() preserves auth credentials on WHATWG URL",
  fn() {
    // https://github.com/denoland/deno/issues/34925
    const u = new URL("https://username:password@example.com/my/path");
    assertEquals(
      format(u),
      "https://username:password@example.com/my/path",
    );

    // Username only.
    assertEquals(
      format(new URL("http://user@example.com/")),
      "http://user@example.com/",
    );

    // Password only.
    assertEquals(
      format(new URL("http://:pass@example.com/")),
      "http://:pass@example.com/",
    );

    // Empty options object should behave like no options.
    assertEquals(
      format(u, {} as never),
      "https://username:password@example.com/my/path",
    );
  },
});

Deno.test({
  name: "[node/url] format() WHATWG URL with auth/fragment/search/unicode",
  fn() {
    const u = new URL(
      "http://user:pass@xn--lck1c3crb1723bpq4a.com/a?a=b#c",
    );

    assertEquals(
      format(u),
      "http://user:pass@xn--lck1c3crb1723bpq4a.com/a?a=b#c",
    );

    // auth: false strips credentials.
    assertEquals(
      format(u, { auth: false } as never),
      "http://xn--lck1c3crb1723bpq4a.com/a?a=b#c",
    );

    // fragment: false strips the hash.
    assertEquals(
      format(u, { fragment: false } as never),
      "http://user:pass@xn--lck1c3crb1723bpq4a.com/a?a=b",
    );

    // search: false strips the query.
    assertEquals(
      format(u, { search: false } as never),
      "http://user:pass@xn--lck1c3crb1723bpq4a.com/a#c",
    );

    // unicode: true decodes punycoded hosts.
    assertEquals(
      format(u, { unicode: true } as never),
      "http://user:pass@理容ナカムラ.com/a?a=b#c",
    );

    // Port is preserved with unicode hostnames.
    assertEquals(
      format(new URL("http://user:pass@xn--0zwm56d.com:8080/path"), {
        unicode: true,
      } as never),
      "http://user:pass@测试.com:8080/path",
    );
  },
});

Deno.test({
  name: "[node/url] format() throws on non-object options",
  fn() {
    const u = new URL("http://example.com/");
    for (const value of [true, 1, "test", Infinity]) {
      assertThrows(
        () => format(u, value as never),
        TypeError,
      );
    }
  },
});
