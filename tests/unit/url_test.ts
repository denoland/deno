// Copyright 2018-2025 the Deno authors. MIT license.
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrows,
} from "./test_util.ts";

Deno.test(function urlParsing() {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
  assertEquals(url.hash, "#qat");
  assertEquals(url.host, "baz.qat:8000");
  assertEquals(url.hostname, "baz.qat");
  assertEquals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
  assertEquals(url.origin, "https://baz.qat:8000");
  assertEquals(url.password, "bar");
  assertEquals(url.pathname, "/qux/quux");
  assertEquals(url.port, "8000");
  assertEquals(url.protocol, "https:");
  assertEquals(url.search, "?foo=bar&baz=12");
  assertEquals(url.searchParams.getAll("foo"), ["bar"]);
  assertEquals(url.searchParams.getAll("baz"), ["12"]);
  assertEquals(url.username, "foo");
  assertEquals(
    String(url),
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
});

Deno.test(function emptyUrl() {
  assertThrows(
    // @ts-ignore for test
    () => new URL(),
    TypeError,
    "1 argument required, but only 0 present",
  );
  assertThrows(
    // @ts-ignore for test
    () => URL.canParse(),
    TypeError,
    "1 argument required, but only 0 present",
  );
});

Deno.test(function urlProtocolParsing() {
  assertEquals(new URL("Aa+-.1://foo").protocol, "aa+-.1:");
  assertEquals(new URL("aA+-.1://foo").protocol, "aa+-.1:");
  assertThrows(() => new URL("1://foo"), TypeError, "Invalid URL: '1://foo'");
  assertThrows(() => new URL("+://foo"), TypeError, "Invalid URL: '+://foo'");
  assertThrows(() => new URL("-://foo"), TypeError, "Invalid URL: '-://foo'");
  assertThrows(() => new URL(".://foo"), TypeError, "Invalid URL: '.://foo'");
  assertThrows(() => new URL("_://foo"), TypeError, "Invalid URL: '_://foo'");
  assertThrows(() => new URL("=://foo"), TypeError, "Invalid URL: '=://foo'");
  assertThrows(() => new URL("!://foo"), TypeError, "Invalid URL: '!://foo'");
  assertThrows(() => new URL(`"://foo`), TypeError, `Invalid URL: '"://foo'`);
  assertThrows(() => new URL("$://foo"), TypeError, "Invalid URL: '$://foo'");
  assertThrows(() => new URL("%://foo"), TypeError, "Invalid URL: '%://foo'");
  assertThrows(() => new URL("^://foo"), TypeError, "Invalid URL: '^://foo'");
  assertThrows(() => new URL("*://foo"), TypeError, "Invalid URL: '*://foo'");
  assertThrows(() => new URL("*://foo"), TypeError, "Invalid URL: '*://foo'");
  assertThrows(
    () => new URL("!:", "*://foo"),
    TypeError,
    "Invalid URL: '!:' with base '*://foo'",
  );
});

Deno.test(function urlAuthenticationParsing() {
  const specialUrl = new URL("http://foo:bar@baz");
  assertEquals(specialUrl.username, "foo");
  assertEquals(specialUrl.password, "bar");
  assertEquals(specialUrl.hostname, "baz");
  assertThrows(() => new URL("file://foo:bar@baz"), TypeError, "Invalid URL");
  const nonSpecialUrl = new URL("abcd://foo:bar@baz");
  assertEquals(nonSpecialUrl.username, "foo");
  assertEquals(nonSpecialUrl.password, "bar");
  assertEquals(nonSpecialUrl.hostname, "baz");
});

Deno.test(function urlHostnameParsing() {
  // IPv6.
  assertEquals(new URL("http://[::1]").hostname, "[::1]");
  assertEquals(new URL("file://[::1]").hostname, "[::1]");
  assertEquals(new URL("abcd://[::1]").hostname, "[::1]");
  assertEquals(new URL("http://[0:f:0:0:f:f:0:0]").hostname, "[0:f::f:f:0:0]");

  // Forbidden host code point.
  assertThrows(() => new URL("http:// a"), TypeError, "Invalid URL");
  assertThrows(() => new URL("file:// a"), TypeError, "Invalid URL");
  assertThrows(() => new URL("abcd:// a"), TypeError, "Invalid URL");
  assertThrows(() => new URL("http://%"), TypeError, "Invalid URL");
  assertThrows(() => new URL("file://%"), TypeError, "Invalid URL");
  assertEquals(new URL("abcd://%").hostname, "%");

  // Percent-decode.
  assertEquals(new URL("http://%21").hostname, "!");
  assertEquals(new URL("file://%21").hostname, "!");
  assertEquals(new URL("abcd://%21").hostname, "%21");

  // IPv4 parsing.
  assertEquals(new URL("http://260").hostname, "0.0.1.4");
  assertEquals(new URL("file://260").hostname, "0.0.1.4");
  assertEquals(new URL("abcd://260").hostname, "260");
  assertEquals(new URL("http://255.0.0.0").hostname, "255.0.0.0");
  assertThrows(() => new URL("http://256.0.0.0"), TypeError, "Invalid URL");
  assertEquals(new URL("http://0.255.0.0").hostname, "0.255.0.0");
  assertThrows(() => new URL("http://0.256.0.0"), TypeError, "Invalid URL");
  assertEquals(new URL("http://0.0.255.0").hostname, "0.0.255.0");
  assertThrows(() => new URL("http://0.0.256.0"), TypeError, "Invalid URL");
  assertEquals(new URL("http://0.0.0.255").hostname, "0.0.0.255");
  assertThrows(() => new URL("http://0.0.0.256"), TypeError, "Invalid URL");
  assertEquals(new URL("http://0.0.65535").hostname, "0.0.255.255");
  assertThrows(() => new URL("http://0.0.65536"), TypeError, "Invalid URL");
  assertEquals(new URL("http://0.16777215").hostname, "0.255.255.255");
  assertThrows(() => new URL("http://0.16777216"), TypeError, "Invalid URL");
  assertEquals(new URL("http://4294967295").hostname, "255.255.255.255");
  assertThrows(() => new URL("http://4294967296"), TypeError, "Invalid URL");
});

Deno.test(function urlPortParsing() {
  const specialUrl = new URL("http://foo:8000");
  assertEquals(specialUrl.hostname, "foo");
  assertEquals(specialUrl.port, "8000");
  assertThrows(() => new URL("file://foo:8000"), TypeError, "Invalid URL");
  const nonSpecialUrl = new URL("abcd://foo:8000");
  assertEquals(nonSpecialUrl.hostname, "foo");
  assertEquals(nonSpecialUrl.port, "8000");
});

Deno.test(function urlModifications() {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
  url.hash = "";
  assertEquals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12",
  );
  url.host = "qat.baz:8080";
  assertEquals(
    url.href,
    "https://foo:bar@qat.baz:8080/qux/quux?foo=bar&baz=12",
  );
  url.hostname = "foo.bar";
  assertEquals(
    url.href,
    "https://foo:bar@foo.bar:8080/qux/quux?foo=bar&baz=12",
  );
  url.password = "qux";
  assertEquals(
    url.href,
    "https://foo:qux@foo.bar:8080/qux/quux?foo=bar&baz=12",
  );
  url.pathname = "/foo/bar%qat";
  assertEquals(
    url.href,
    "https://foo:qux@foo.bar:8080/foo/bar%qat?foo=bar&baz=12",
  );
  url.port = "";
  assertEquals(url.href, "https://foo:qux@foo.bar/foo/bar%qat?foo=bar&baz=12");
  url.protocol = "http:";
  assertEquals(url.href, "http://foo:qux@foo.bar/foo/bar%qat?foo=bar&baz=12");
  url.search = "?foo=bar&foo=baz";
  assertEquals(url.href, "http://foo:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz");
  assertEquals(url.searchParams.getAll("foo"), ["bar", "baz"]);
  url.username = "foo@bar";
  assertEquals(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz",
  );
  url.searchParams.set("bar", "qat");
  assertEquals(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz&bar=qat",
  );
  url.searchParams.delete("foo");
  assertEquals(url.href, "http://foo%40bar:qux@foo.bar/foo/bar%qat?bar=qat");
  url.searchParams.append("foo", "bar");
  assertEquals(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?bar=qat&foo=bar",
  );
});

Deno.test(function urlModifyHref() {
  const url = new URL("http://example.com/");
  url.href = "https://foo:bar@example.com:8080/baz/qat#qux";
  assertEquals(url.protocol, "https:");
  assertEquals(url.username, "foo");
  assertEquals(url.password, "bar");
  assertEquals(url.host, "example.com:8080");
  assertEquals(url.hostname, "example.com");
  assertEquals(url.pathname, "/baz/qat");
  assertEquals(url.hash, "#qux");
});

Deno.test(function urlNormalize() {
  const url = new URL("http://example.com");
  assertEquals(url.pathname, "/");
  assertEquals(url.href, "http://example.com/");
});

Deno.test(function urlModifyPathname() {
  const url = new URL("http://foo.bar/baz%qat/qux%quux");
  assertEquals(url.pathname, "/baz%qat/qux%quux");
  // Self-assignment is to invoke the setter.
  // deno-lint-ignore no-self-assign
  url.pathname = url.pathname;
  assertEquals(url.pathname, "/baz%qat/qux%quux");
  url.pathname = "baz#qat qux";
  assertEquals(url.pathname, "/baz%23qat%20qux");
  // deno-lint-ignore no-self-assign
  url.pathname = url.pathname;
  assertEquals(url.pathname, "/baz%23qat%20qux");
  url.pathname = "\\a\\b\\c";
  assertEquals(url.pathname, "/a/b/c");
});

Deno.test(function urlModifyHash() {
  const url = new URL("http://foo.bar");
  url.hash = "%foo bar/qat%qux#bar";
  assertEquals(url.hash, "#%foo%20bar/qat%qux#bar");
  // deno-lint-ignore no-self-assign
  url.hash = url.hash;
  assertEquals(url.hash, "#%foo%20bar/qat%qux#bar");
});

Deno.test(function urlSearchParamsReuse() {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
  const sp = url.searchParams;
  url.host = "baz.qat";
  assert(sp === url.searchParams, "Search params should be reused.");
});

Deno.test(function urlBackSlashes() {
  const url = new URL(
    "https:\\\\foo:bar@baz.qat:8000\\qux\\quux?foo=bar&baz=12#qat",
  );
  assertEquals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
});

Deno.test(function urlProtocolSlashes() {
  assertEquals(new URL("http:foo").href, "http://foo/");
  assertEquals(new URL("http://foo").href, "http://foo/");
  assertEquals(new URL("file:foo").href, "file:///foo");
  assertEquals(new URL("file://foo").href, "file://foo/");
  assertEquals(new URL("abcd:foo").href, "abcd:foo");
  assertEquals(new URL("abcd://foo").href, "abcd://foo");
});

Deno.test(function urlRequireHost() {
  assertEquals(new URL("file:///").href, "file:///");
  assertThrows(() => new URL("ftp:///"), TypeError, "Invalid URL");
  assertThrows(() => new URL("http:///"), TypeError, "Invalid URL");
  assertThrows(() => new URL("https:///"), TypeError, "Invalid URL");
  assertThrows(() => new URL("ws:///"), TypeError, "Invalid URL");
  assertThrows(() => new URL("wss:///"), TypeError, "Invalid URL");
});

Deno.test(function urlDriveLetter() {
  assertEquals(new URL("file:///C:").href, "file:///C:");
  assertEquals(new URL("file:///C:/").href, "file:///C:/");
  assertEquals(new URL("file:///C:/..").href, "file:///C:/");

  // Don't recognise drive letters with extra leading slashes.
  // FIXME(nayeemrmn): This is true according to
  // https://jsdom.github.io/whatwg-url/#url=ZmlsZTovLy8vQzovLi4=&base=ZmlsZTovLy8=
  // but not the behavior of rust-url.
  // assertEquals(new URL("file:////C:/..").href, "file:///");

  // Drop the hostname if a drive letter is parsed.
  assertEquals(new URL("file://foo/C:").href, "file:///C:");

  // Don't recognise drive letters in non-file protocols.
  // FIXME(nayeemrmn): This is true according to
  // https://jsdom.github.io/whatwg-url/#url=YWJjZDovL2Zvby9DOi8uLg==&base=ZmlsZTovLy8=
  // but not the behavior of rust-url.
  // assertEquals(new URL("http://foo/C:/..").href, "http://foo/");
  // assertEquals(new URL("abcd://foo/C:/..").href, "abcd://foo/");
});

Deno.test(function urlHostnameUpperCase() {
  assertEquals(new URL("http://EXAMPLE.COM").href, "http://example.com/");
  assertEquals(new URL("abcd://EXAMPLE.COM").href, "abcd://EXAMPLE.COM");
});

Deno.test(function urlEmptyPath() {
  assertEquals(new URL("http://foo").pathname, "/");
  assertEquals(new URL("file://foo").pathname, "/");
  assertEquals(new URL("abcd://foo").pathname, "");
});

Deno.test(function urlPathRepeatedSlashes() {
  assertEquals(new URL("http://foo//bar//").pathname, "//bar//");
  assertEquals(new URL("file://foo///bar//").pathname, "/bar//");
  assertEquals(new URL("abcd://foo//bar//").pathname, "//bar//");
});

Deno.test(function urlTrim() {
  assertEquals(new URL(" http://example.com  ").href, "http://example.com/");
});

Deno.test(function urlEncoding() {
  assertEquals(
    new URL("http://a !$&*()=,;+'\"@example.com").username,
    "a%20!$&*()%3D,%3B+'%22",
  );
  assertEquals(
    new URL("http://:a !$&*()=,;+'\"@example.com").password,
    "a%20!$&*()%3D,%3B+'%22",
  );
  // https://url.spec.whatwg.org/#idna
  assertEquals(new URL("http://mañana/c?d#e").hostname, "xn--maana-pta");
  assertEquals(new URL("abcd://mañana/c?d#e").hostname, "ma%C3%B1ana");
  assertEquals(
    new URL("http://example.com/a ~!@$&*()=:/,;+'\"\\").pathname,
    "/a%20~!@$&*()=:/,;+'%22/",
  );
  assertEquals(
    new URL("http://example.com?a ~!@$&*()=:/,;?+'\"\\").search,
    "?a%20~!@$&*()=:/,;?+%27%22\\",
  );
  assertEquals(
    new URL("abcd://example.com?a ~!@$&*()=:/,;?+'\"\\").search,
    "?a%20~!@$&*()=:/,;?+'%22\\",
  );
  assertEquals(
    new URL("http://example.com#a ~!@#$&*()=:/,;?+'\"\\").hash,
    "#a%20~!@#$&*()=:/,;?+'%22\\",
  );
});

Deno.test(function urlBase() {
  assertEquals(new URL("d", new URL("http://foo/a?b#c")).href, "http://foo/d");

  assertEquals(new URL("", "http://foo/a/b?c#d").href, "http://foo/a/b?c");
  assertEquals(new URL("", "file://foo/a/b?c#d").href, "file://foo/a/b?c");
  assertEquals(new URL("", "abcd://foo/a/b?c#d").href, "abcd://foo/a/b?c");

  assertEquals(new URL("#e", "http://foo/a/b?c#d").href, "http://foo/a/b?c#e");
  assertEquals(new URL("#e", "file://foo/a/b?c#d").href, "file://foo/a/b?c#e");
  assertEquals(new URL("#e", "abcd://foo/a/b?c#d").href, "abcd://foo/a/b?c#e");

  assertEquals(new URL("?e", "http://foo/a/b?c#d").href, "http://foo/a/b?e");
  assertEquals(new URL("?e", "file://foo/a/b?c#d").href, "file://foo/a/b?e");
  assertEquals(new URL("?e", "abcd://foo/a/b?c#d").href, "abcd://foo/a/b?e");

  assertEquals(new URL("e", "http://foo/a/b?c#d").href, "http://foo/a/e");
  assertEquals(new URL("e", "file://foo/a/b?c#d").href, "file://foo/a/e");
  assertEquals(new URL("e", "abcd://foo/a/b?c#d").href, "abcd://foo/a/e");

  assertEquals(new URL(".", "http://foo/a/b?c#d").href, "http://foo/a/");
  assertEquals(new URL(".", "file://foo/a/b?c#d").href, "file://foo/a/");
  assertEquals(new URL(".", "abcd://foo/a/b?c#d").href, "abcd://foo/a/");

  assertEquals(new URL("..", "http://foo/a/b?c#d").href, "http://foo/");
  assertEquals(new URL("..", "file://foo/a/b?c#d").href, "file://foo/");
  assertEquals(new URL("..", "abcd://foo/a/b?c#d").href, "abcd://foo/");

  assertEquals(new URL("/e", "http://foo/a/b?c#d").href, "http://foo/e");
  assertEquals(new URL("/e", "file://foo/a/b?c#d").href, "file://foo/e");
  assertEquals(new URL("/e", "abcd://foo/a/b?c#d").href, "abcd://foo/e");

  assertEquals(new URL("//bar", "http://foo/a/b?c#d").href, "http://bar/");
  assertEquals(new URL("//bar", "file://foo/a/b?c#d").href, "file://bar/");
  assertEquals(new URL("//bar", "abcd://foo/a/b?c#d").href, "abcd://bar");

  assertEquals(new URL("efgh:", "http://foo/a/b?c#d").href, "efgh:");
  assertEquals(new URL("efgh:", "file://foo/a/b?c#d").href, "efgh:");
  assertEquals(new URL("efgh:", "abcd://foo/a/b?c#d").href, "efgh:");

  assertEquals(new URL("/foo", "abcd:/").href, "abcd:/foo");
});

Deno.test(function urlDriveLetterBase() {
  assertEquals(new URL("/b", "file:///C:/a/b").href, "file:///C:/b");
  assertEquals(new URL("/D:", "file:///C:/a/b").href, "file:///D:");
});

Deno.test(function urlSameProtocolBase() {
  assertEquals(new URL("http:", "http://foo/a").href, "http://foo/a");
  assertEquals(new URL("file:", "file://foo/a").href, "file://foo/a");
  assertEquals(new URL("abcd:", "abcd://foo/a").href, "abcd:");

  assertEquals(new URL("http:b", "http://foo/a").href, "http://foo/b");
  assertEquals(new URL("file:b", "file://foo/a").href, "file://foo/b");
  assertEquals(new URL("abcd:b", "abcd://foo/a").href, "abcd:b");
});

Deno.test(function deletingAllParamsRemovesQuestionMarkFromURL() {
  const url = new URL("http://example.com/?param1&param2");
  url.searchParams.delete("param1");
  url.searchParams.delete("param2");
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});

Deno.test(function removingNonExistentParamRemovesQuestionMarkFromURL() {
  const url = new URL("http://example.com/?");
  assertEquals(url.href, "http://example.com/?");
  url.searchParams.delete("param1");
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});

Deno.test(function sortingNonExistentParamRemovesQuestionMarkFromURL() {
  const url = new URL("http://example.com/?");
  assertEquals(url.href, "http://example.com/?");
  url.searchParams.sort();
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});

Deno.test(function customInspectFunction() {
  const url = new URL("http://example.com/?");
  assertEquals(
    Deno.inspect(url),
    `URL {
  href: "http://example.com/?",
  origin: "http://example.com",
  protocol: "http:",
  username: "",
  password: "",
  host: "example.com",
  hostname: "example.com",
  port: "",
  pathname: "/",
  hash: "",
  search: ""
}`,
  );
});

Deno.test(function protocolNotHttpOrFile() {
  const url = new URL("about:blank");
  assertEquals(url.href, "about:blank");
  assertEquals(url.protocol, "about:");
  assertEquals(url.origin, "null");
});

Deno.test(function throwForInvalidPortConstructor() {
  const urls = [
    // If port is greater than 2^16 − 1, validation error, return failure.
    `https://baz.qat:${2 ** 16}`,
    "https://baz.qat:-32",
    "https://baz.qat:deno",
    "https://baz.qat:9land",
    "https://baz.qat:10.5",
  ];

  for (const url of urls) {
    assertThrows(() => new URL(url), TypeError, "Invalid URL");
  }

  // Do not throw for 0 & 65535
  new URL("https://baz.qat:65535");
  new URL("https://baz.qat:0");
});

Deno.test(function doNotOverridePortIfInvalid() {
  const initialPort = "3000";
  const url = new URL(`https://deno.land:${initialPort}`);
  // If port is greater than 2^16 − 1, validation error, return failure.
  url.port = `${2 ** 16}`;
  assertEquals(url.port, initialPort);
});

Deno.test(function emptyPortForSchemeDefaultPort() {
  const nonDefaultPort = "3500";

  const url = new URL("ftp://baz.qat:21");
  assertEquals(url.port, "");
  url.port = nonDefaultPort;
  assertEquals(url.port, nonDefaultPort);
  url.port = "21";
  assertEquals(url.port, "");
  url.protocol = "http";
  assertEquals(url.port, "");

  const url2 = new URL("https://baz.qat:443");
  assertEquals(url2.port, "");
  url2.port = nonDefaultPort;
  assertEquals(url2.port, nonDefaultPort);
  url2.port = "443";
  assertEquals(url2.port, "");
  url2.protocol = "http";
  assertEquals(url2.port, "");
});

Deno.test(function assigningPortPropertyAffectsReceiverOnly() {
  // Setting `.port` should update only the receiver.
  const u1 = new URL("http://google.com/");
  // deno-lint-ignore no-explicit-any
  const u2 = new URL(u1 as any);
  u2.port = "123";
  assertStrictEquals(u1.port, "");
  assertStrictEquals(u2.port, "123");
});

Deno.test(function urlSearchParamsIdentityPreserved() {
  // URLSearchParams identity should not be lost when URL is updated.
  const u = new URL("http://foo.com/");
  const sp1 = u.searchParams;
  u.href = "http://bar.com/?baz=42";
  const sp2 = u.searchParams;
  assertStrictEquals(sp1, sp2);
});

Deno.test(function urlTakeURLObjectAsParameter() {
  const url = new URL(
    new URL(
      "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
    ),
  );
  assertEquals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
});
