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
    // Forbidden domain code points (control chars, space, and characters like
    // `%<>`) are rejected and return an empty string, matching Node.js.
    // Regression test for https://github.com/denoland/deno/issues/36514
    assertEquals(domainToASCII("\x00null.com"), "");
    assertEquals(domainToASCII("a b.com"), "");
    assertEquals(domainToASCII("a<b.com"), "");
    assertEquals(domainToASCII("a%b.com"), "");
    // Underscores and empty labels are allowed (not forbidden code points).
    assertEquals(domainToASCII("under_score.com"), "under_score.com");
    assertEquals(domainToASCII("example..com"), "example..com");
    // `domainToASCII` runs the full WHATWG URL host parser, not just UTS #46
    // ToASCII. The host terminates at the first `/`, `\`, `?` or `#` ...
    assertEquals(domainToASCII("host#name"), "host");
    assertEquals(domainToASCII("host?name"), "host");
    assertEquals(domainToASCII("host/name"), "host");
    assertEquals(domainToASCII("host\\name"), "host");
    // ... ASCII tab/newline code points are stripped ...
    assertEquals(domainToASCII("ho\tst\nna\rme"), "hostname");
    // ... the remainder is percent-decoded before ToASCII ...
    assertEquals(domainToASCII("%41"), "a");
    assertEquals(domainToASCII("host%2f"), "");
    // ... and IPv4/IPv6 literals are parsed and normalized.
    assertEquals(domainToASCII("0xffffffff"), "255.255.255.255");
    assertEquals(domainToASCII("[0:0:0:0:0:0:0:1]"), "[::1]");
    assertEquals(domainToASCII("[::ffff:1.2.3.4]"), "[::ffff:102:304]");
    // A bare (unbracketed) IPv6 literal contains `:` and is rejected.
    assertEquals(domainToASCII("2001:4860:4860::8888"), "");
    // An unterminated / invalid bracketed literal is rejected.
    assertEquals(domainToASCII("[::1]extra"), "");
    assertEquals(domainToASCII("[garbage]"), "");
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
