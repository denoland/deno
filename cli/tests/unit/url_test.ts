// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals, assertThrows } from "./test_util.ts";

unitTest(function urlParsing(): void {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assertEquals(url.hash, "#qat");
  assertEquals(url.host, "baz.qat:8000");
  assertEquals(url.hostname, "baz.qat");
  assertEquals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
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
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
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
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  url.hash = "";
  assertEquals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12"
  );
  url.host = "qat.baz:8080";
  assertEquals(
    url.href,
    "https://foo:bar@qat.baz:8080/qux/quux?foo=bar&baz=12"
  );
  url.hostname = "foo.bar";
  assertEquals(
    url.href,
    "https://foo:bar@foo.bar:8080/qux/quux?foo=bar&baz=12"
  );
  url.password = "qux";
  assertEquals(
    url.href,
    "https://foo:qux@foo.bar:8080/qux/quux?foo=bar&baz=12"
  );
  url.pathname = "/foo/bar%qat";
  assertEquals(
    url.href,
    "https://foo:qux@foo.bar:8080/foo/bar%qat?foo=bar&baz=12"
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
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz"
  );
  url.searchParams.set("bar", "qat");
  assertEquals(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz&bar=qat"
  );
  url.searchParams.delete("foo");
  assertEquals(url.href, "http://foo%40bar:qux@foo.bar/foo/bar%qat?bar=qat");
  url.searchParams.append("foo", "bar");
  assertEquals(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?bar=qat&foo=bar"
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
  url.pathname = url.pathname;
  assertEquals(url.pathname, "/baz%qat/qux%quux");
  url.pathname = "baz#qat qux";
  assertEquals(url.pathname, "/baz%23qat%20qux");
  url.pathname = url.pathname;
  assertEquals(url.pathname, "/baz%23qat%20qux");
});

unitTest(function urlModifyHash(): void {
  const url = new URL("http://foo.bar");
  url.hash = "%foo bar/qat%qux#bar";
  assertEquals(url.hash, "#%foo%20bar/qat%qux#bar");
  url.hash = url.hash;
  assertEquals(url.hash, "#%foo%20bar/qat%qux#bar");
});

unitTest(function urlSearchParamsReuse(): void {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  const sp = url.searchParams;
  url.host = "baz.qat";
  assert(sp === url.searchParams, "Search params should be reused.");
});

unitTest(function urlBackSlashes(): void {
  const url = new URL(
    "https:\\\\foo:bar@baz.qat:8000\\qux\\quux?foo=bar&baz=12#qat"
  );
  assertEquals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
});

unitTest(function urlRequireHost(): void {
  assertEquals(new URL("file:///").href, "file:///");
  assertThrows(() => {
    new URL("ftp:///");
  });
  assertThrows(() => {
    new URL("http:///");
  });
  assertThrows(() => {
    new URL("https:///");
  });
  assertThrows(() => {
    new URL("ws:///");
  });
  assertThrows(() => {
    new URL("wss:///");
  });
});

unitTest(function urlDriveLetter() {
  assertEquals(
    new URL("file:///C:").href,
    Deno.build.os == "windows" ? "file:///C:/" : "file:///C:"
  );
  assertEquals(new URL("http://example.com/C:").href, "http://example.com/C:");
});

unitTest(function urlUncHostname() {
  assertEquals(
    new URL("file:////").href,
    Deno.build.os == "windows" ? "file:///" : "file:////"
  );
  assertEquals(
    new URL("file:////server").href,
    Deno.build.os == "windows" ? "file://server/" : "file:////server"
  );
  assertEquals(
    new URL("file:////server/file").href,
    Deno.build.os == "windows" ? "file://server/file" : "file:////server/file"
  );
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

unitTest(function urlTrim() {
  assertEquals(new URL(" http://example.com  ").href, "http://example.com/");
});

unitTest(function urlEncoding() {
  assertEquals(
    new URL("https://a !$&*()=,;+'\"@example.com").username,
    "a%20!$&*()%3D,%3B+%27%22"
  );
  assertEquals(
    new URL("https://:a !$&*()=,;+'\"@example.com").password,
    "a%20!$&*()%3D,%3B+%27%22"
  );
  assertEquals(new URL("abcde://mañana/c?d#e").hostname, "ma%C3%B1ana");
  // https://url.spec.whatwg.org/#idna
  assertEquals(new URL("https://mañana/c?d#e").hostname, "xn--maana-pta");
  assertEquals(
    new URL("https://example.com/a ~!@$&*()=:/,;+'\"\\").pathname,
    "/a%20~!@$&*()=:/,;+'%22/"
  );
  assertEquals(
    new URL("https://example.com?a ~!@$&*()=:/,;?+'\"\\").search,
    "?a%20~!@$&*()=:/,;?+%27%22\\"
  );
  assertEquals(
    new URL("https://example.com#a ~!@#$&*()=:/,;?+'\"\\").hash,
    "#a%20~!@#$&*()=:/,;?+'%22\\"
  );
});

unitTest(function urlBaseURL(): void {
  const base = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  const url = new URL("/foo/bar?baz=foo#qux", base);
  assertEquals(url.href, "https://foo:bar@baz.qat:8000/foo/bar?baz=foo#qux");

  assertEquals(
    new URL("D", "https://foo.bar/path/a/b/c/d").href,
    "https://foo.bar/path/a/b/c/D"
  );

  assertEquals(new URL("D", "https://foo.bar").href, "https://foo.bar/D");
  assertEquals(new URL("D", "https://foo.bar/").href, "https://foo.bar/D");

  assertEquals(
    new URL("/d", "https://foo.bar/path/a/b/c/d").href,
    "https://foo.bar/d"
  );
});

unitTest(function urlBaseString(): void {
  const url = new URL(
    "/foo/bar?baz=foo#qux",
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assertEquals(url.href, "https://foo:bar@baz.qat:8000/foo/bar?baz=foo#qux");
});

unitTest(function urlRelativeWithBase(): void {
  assertEquals(new URL("", "file:///a/a/a").href, "file:///a/a/a");
  assertEquals(new URL(".", "file:///a/a/a").href, "file:///a/a/");
  assertEquals(new URL("..", "file:///a/a/a").href, "file:///a/");
  assertEquals(new URL("b", "file:///a/a/a").href, "file:///a/a/b");
  assertEquals(new URL("b", "file:///a/a/a/").href, "file:///a/a/a/b");
  assertEquals(new URL("b/", "file:///a/a/a").href, "file:///a/a/b/");
  assertEquals(new URL("../b", "file:///a/a/a").href, "file:///a/b");
});

unitTest(function urlDriveLetterBase() {
  assertEquals(
    new URL("/b", "file:///C:/a/b").href,
    Deno.build.os == "windows" ? "file:///C:/b" : "file:///b"
  );
  assertEquals(
    new URL("D:", "file:///C:/a/b").href,
    Deno.build.os == "windows" ? "file:///D:/" : "file:///C:/a/D:"
  );
  assertEquals(
    new URL("/D:", "file:///C:/a/b").href,
    Deno.build.os == "windows" ? "file:///D:/" : "file:///D:"
  );
  assertEquals(
    new URL("D:/b", "file:///C:/a/b").href,
    Deno.build.os == "windows" ? "file:///D:/b" : "file:///C:/a/D:/b"
  );
});

unitTest(function emptyBasePath(): void {
  assertEquals(new URL("", "http://example.com").href, "http://example.com/");
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
      'URL { href: "http://example.com/?", origin: "http://example.com", protocol: "http:", username: "", password: "", host: "example.com", hostname: "example.com", port: "", pathname: "/", hash: "", search: "?" }'
    );
  }
);

unitTest(function protocolNotHttpOrFile() {
  const url = new URL("about:blank");
  assertEquals(url.href, "about:blank");
  assertEquals(url.protocol, "about:");
  assertEquals(url.origin, "null");
});

unitTest(function createBadUrl(): void {
  assertThrows(() => {
    new URL("0.0.0.0:8080");
  });
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

unitTest(function throwForInvalidSchemeConstructor(): void {
  assertThrows(
    () => new URL("invalid_scheme://baz.qat"),
    TypeError,
    "Invalid URL."
  );
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
