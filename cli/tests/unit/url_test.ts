// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows, unitTest } from "./test_util.ts";

unitTest(function urlParsing(): void {
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

unitTest(function urlProtocolParsing(): void {
  assertEquals(new URL("Aa+-.1://foo").protocol, "aa+-.1:");
  assertEquals(new URL("aA+-.1://foo").protocol, "aa+-.1:");
  assertThrows(() => new URL("1://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("+://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("-://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL(".://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("_://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("=://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("!://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL(`"://foo`), TypeError, "Invalid URL.");
  assertThrows(() => new URL("$://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("%://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("^://foo"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("*://foo"), TypeError, "Invalid URL.");
});

unitTest(function urlAuthenticationParsing(): void {
  const specialUrl = new URL("http://foo:bar@baz");
  assertEquals(specialUrl.username, "foo");
  assertEquals(specialUrl.password, "bar");
  assertEquals(specialUrl.hostname, "baz");
  assertThrows(() => new URL("file://foo:bar@baz"), TypeError, "Invalid URL.");
  const nonSpecialUrl = new URL("abcd://foo:bar@baz");
  assertEquals(nonSpecialUrl.username, "foo");
  assertEquals(nonSpecialUrl.password, "bar");
  assertEquals(nonSpecialUrl.hostname, "baz");
});

unitTest(function urlHostnameParsing(): void {
  // IPv6.
  assertEquals(new URL("http://[::1]").hostname, "[::1]");
  assertEquals(new URL("file://[::1]").hostname, "[::1]");
  assertEquals(new URL("abcd://[::1]").hostname, "[::1]");
  assertEquals(new URL("http://[0:f:0:0:f:f:0:0]").hostname, "[0:f::f:f:0:0]");
  assertEquals(new URL("http://[0:0:5:6:7:8]").hostname, "[::5:6:7:8]");

  // Forbidden host code point.
  assertThrows(() => new URL("http:// a"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("file:// a"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("abcd:// a"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("http://%"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("file://%"), TypeError, "Invalid URL.");
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
  assertThrows(() => new URL("http://256.0.0.0"), TypeError, "Invalid URL.");
  assertEquals(new URL("http://0.255.0.0").hostname, "0.255.0.0");
  assertThrows(() => new URL("http://0.256.0.0"), TypeError, "Invalid URL.");
  assertEquals(new URL("http://0.0.255.0").hostname, "0.0.255.0");
  assertThrows(() => new URL("http://0.0.256.0"), TypeError, "Invalid URL.");
  assertEquals(new URL("http://0.0.0.255").hostname, "0.0.0.255");
  assertThrows(() => new URL("http://0.0.0.256"), TypeError, "Invalid URL.");
  assertEquals(new URL("http://0.0.65535").hostname, "0.0.255.255");
  assertThrows(() => new URL("http://0.0.65536"), TypeError, "Invalid URL.");
  assertEquals(new URL("http://0.16777215").hostname, "0.255.255.255");
  assertThrows(() => new URL("http://0.16777216"), TypeError, "Invalid URL.");
  assertEquals(new URL("http://4294967295").hostname, "255.255.255.255");
  assertThrows(() => new URL("http://4294967296"), TypeError, "Invalid URL.");
});

unitTest(function urlPortParsing(): void {
  const specialUrl = new URL("http://foo:8000");
  assertEquals(specialUrl.hostname, "foo");
  assertEquals(specialUrl.port, "8000");
  assertThrows(() => new URL("file://foo:8000"), TypeError, "Invalid URL.");
  const nonSpecialUrl = new URL("abcd://foo:8000");
  assertEquals(nonSpecialUrl.hostname, "foo");
  assertEquals(nonSpecialUrl.port, "8000");
});

unitTest(function urlModifications(): void {
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

unitTest(function urlModifyHref(): void {
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

unitTest(function urlNormalize(): void {
  const url = new URL("http://example.com");
  assertEquals(url.pathname, "/");
  assertEquals(url.href, "http://example.com/");
});

unitTest(function urlModifyPathname(): void {
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

unitTest(function urlModifyHash(): void {
  const url = new URL("http://foo.bar");
  url.hash = "%foo bar/qat%qux#bar";
  assertEquals(url.hash, "#%foo%20bar/qat%qux#bar");
  // deno-lint-ignore no-self-assign
  url.hash = url.hash;
  assertEquals(url.hash, "#%foo%20bar/qat%qux#bar");
});

unitTest(function urlSearchParamsReuse(): void {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
  const sp = url.searchParams;
  url.host = "baz.qat";
  assert(sp === url.searchParams, "Search params should be reused.");
});

unitTest(function urlBackSlashes(): void {
  const url = new URL(
    "https:\\\\foo:bar@baz.qat:8000\\qux\\quux?foo=bar&baz=12#qat",
  );
  assertEquals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat",
  );
});

unitTest(function urlProtocolSlashes(): void {
  assertEquals(new URL("http:foo").href, "http://foo/");
  assertEquals(new URL("http://foo").href, "http://foo/");
  assertEquals(new URL("file:foo").href, "file:///foo");
  assertEquals(new URL("file://foo").href, "file://foo/");
  assertEquals(new URL("abcd:foo").href, "abcd:foo");
  assertEquals(new URL("abcd://foo").href, "abcd://foo");
});

unitTest(function urlRequireHost(): void {
  assertEquals(new URL("file:///").href, "file:///");
  assertThrows(() => new URL("ftp:///"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("http:///"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("https:///"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("ws:///"), TypeError, "Invalid URL.");
  assertThrows(() => new URL("wss:///"), TypeError, "Invalid URL.");
});

unitTest(function urlDriveLetter() {
  assertEquals(new URL("file:///C:").href, "file:///C:");
  assertEquals(new URL("file:///C:/").href, "file:///C:/");
  assertEquals(new URL("file:///C:/..").href, "file:///C:/");
  // Don't recognise drive letters with extra leading slashes.
  assertEquals(new URL("file:////C:/..").href, "file:///");
  // Drop the hostname if a drive letter is parsed.
  assertEquals(new URL("file://foo/C:").href, "file:///C:");
  // Don't recognise drive letters in non-file protocols.
  assertEquals(new URL("http://foo/C:/..").href, "http://foo/");
  assertEquals(new URL("abcd://foo/C:/..").href, "abcd://foo/");
});

unitTest(function urlHostnameUpperCase() {
  assertEquals(new URL("http://EXAMPLE.COM").href, "http://example.com/");
  assertEquals(new URL("abcd://EXAMPLE.COM").href, "abcd://EXAMPLE.COM");
});

unitTest(function urlEmptyPath() {
  assertEquals(new URL("http://foo").pathname, "/");
  assertEquals(new URL("file://foo").pathname, "/");
  assertEquals(new URL("abcd://foo").pathname, "");
});

unitTest(function urlPathRepeatedSlashes() {
  assertEquals(new URL("http://foo//bar//").pathname, "//bar//");
  assertEquals(new URL("file://foo///bar//").pathname, "/bar//");
  assertEquals(new URL("abcd://foo//bar//").pathname, "//bar//");
});

unitTest(function urlTrim() {
  assertEquals(new URL(" http://example.com  ").href, "http://example.com/");
});

unitTest(function urlEncoding() {
  assertEquals(
    new URL("http://a !$&*()=,;+'\"@example.com").username,
    "a%20!$&*()%3D,%3B+%27%22",
  );
  assertEquals(
    new URL("http://:a !$&*()=,;+'\"@example.com").password,
    "a%20!$&*()%3D,%3B+%27%22",
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

unitTest(function urlBase(): void {
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

unitTest(function urlDriveLetterBase() {
  assertEquals(new URL("/b", "file:///C:/a/b").href, "file:///C:/b");
  assertEquals(new URL("/D:", "file:///C:/a/b").href, "file:///D:");
});

unitTest(function urlSameProtocolBase() {
  assertEquals(new URL("http:", "http://foo/a").href, "http://foo/a");
  assertEquals(new URL("file:", "file://foo/a").href, "file://foo/a");
  assertEquals(new URL("abcd:", "abcd://foo/a").href, "abcd:");

  assertEquals(new URL("http:b", "http://foo/a").href, "http://foo/b");
  assertEquals(new URL("file:b", "file://foo/a").href, "file://foo/b");
  assertEquals(new URL("abcd:b", "abcd://foo/a").href, "abcd:b");
});

unitTest(function deletingAllParamsRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?param1&param2");
  url.searchParams.delete("param1");
  url.searchParams.delete("param2");
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});

unitTest(function removingNonExistentParamRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?");
  assertEquals(url.href, "http://example.com/?");
  url.searchParams.delete("param1");
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});

unitTest(function sortingNonExistentParamRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?");
  assertEquals(url.href, "http://example.com/?");
  url.searchParams.sort();
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});

unitTest(
  {
    // FIXME(bartlomieju)
    ignore: true,
  },
  function customInspectFunction(): void {
    const url = new URL("http://example.com/?");
    assertEquals(
      Deno.inspect(url),
      'URL { href: "http://example.com/?", origin: "http://example.com", protocol: "http:", username: "", password: "", host: "example.com", hostname: "example.com", port: "", pathname: "/", hash: "", search: "?" }',
    );
  },
);

unitTest(function protocolNotHttpOrFile() {
  const url = new URL("about:blank");
  assertEquals(url.href, "about:blank");
  assertEquals(url.protocol, "about:");
  assertEquals(url.origin, "null");
});

unitTest(function throwForInvalidPortConstructor(): void {
  const urls = [
    // If port is greater than 2^16 − 1, validation error, return failure.
    `https://baz.qat:${2 ** 16}`,
    "https://baz.qat:-32",
    "https://baz.qat:deno",
    "https://baz.qat:9land",
    "https://baz.qat:10.5",
  ];

  for (const url of urls) {
    assertThrows(() => new URL(url), TypeError, "Invalid URL.");
  }

  // Do not throw for 0 & 65535
  new URL("https://baz.qat:65535");
  new URL("https://baz.qat:0");
});

unitTest(function doNotOverridePortIfInvalid(): void {
  const initialPort = "3000";
  const ports = [
    // If port is greater than 2^16 − 1, validation error, return failure.
    `${2 ** 16}`,
    "-32",
    "deno",
    "9land",
    "10.5",
  ];

  for (const port of ports) {
    const url = new URL(`https://deno.land:${initialPort}`);
    url.port = port;
    assertEquals(url.port, initialPort);
  }
});

unitTest(function emptyPortForSchemeDefaultPort(): void {
  const nonDefaultPort = "3500";
  const urls = [
    { url: "ftp://baz.qat:21", port: "21", protocol: "ftp:" },
    { url: "https://baz.qat:443", port: "443", protocol: "https:" },
    { url: "wss://baz.qat:443", port: "443", protocol: "wss:" },
    { url: "http://baz.qat:80", port: "80", protocol: "http:" },
    { url: "ws://baz.qat:80", port: "80", protocol: "ws:" },
    { url: "file://home/index.html", port: "", protocol: "file:" },
    { url: "/foo", baseUrl: "ftp://baz.qat:21", port: "21", protocol: "ftp:" },
    {
      url: "/foo",
      baseUrl: "https://baz.qat:443",
      port: "443",
      protocol: "https:",
    },
    {
      url: "/foo",
      baseUrl: "wss://baz.qat:443",
      port: "443",
      protocol: "wss:",
    },
    {
      url: "/foo",
      baseUrl: "http://baz.qat:80",
      port: "80",
      protocol: "http:",
    },
    { url: "/foo", baseUrl: "ws://baz.qat:80", port: "80", protocol: "ws:" },
    {
      url: "/foo",
      baseUrl: "file://home/index.html",
      port: "",
      protocol: "file:",
    },
  ];

  for (const { url: urlString, baseUrl, port, protocol } of urls) {
    const url = new URL(urlString, baseUrl);
    assertEquals(url.port, "");

    url.port = nonDefaultPort;
    assertEquals(url.port, nonDefaultPort);

    url.port = port;
    assertEquals(url.port, "");

    // change scheme
    url.protocol = "sftp:";
    assertEquals(url.port, port);

    url.protocol = protocol;
    assertEquals(url.port, "");
  }
});
