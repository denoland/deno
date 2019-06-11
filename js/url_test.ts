// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEquals } from "./test_util.ts";

test(function urlParsing(): void {
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
  assertEquals(
    JSON.stringify({ key: url }),
    `{"key":"https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"}`
  );
});

test(function urlModifications(): void {
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

test(function urlModifyHref(): void {
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

test(function urlModifyPathname(): void {
  const url = new URL("http://foo.bar/baz%qat/qux%quux");
  assertEquals(url.pathname, "/baz%qat/qux%quux");
  url.pathname = url.pathname;
  assertEquals(url.pathname, "/baz%qat/qux%quux");
  url.pathname = "baz#qat qux";
  assertEquals(url.pathname, "/baz%23qat%20qux");
  url.pathname = url.pathname;
  assertEquals(url.pathname, "/baz%23qat%20qux");
});

test(function urlModifyHash(): void {
  const url = new URL("http://foo.bar");
  url.hash = "%foo bar/qat%qux#bar";
  assertEquals(url.hash, "#%foo%20bar/qat%qux#bar");
  url.hash = url.hash;
  assertEquals(url.hash, "#%foo%20bar/qat%qux#bar");
});

test(function urlSearchParamsReuse(): void {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  const sp = url.searchParams;
  url.host = "baz.qat";
  assert(sp === url.searchParams, "Search params should be reused.");
});

test(function urlBaseURL(): void {
  const base = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  const url = new URL("/foo/bar?baz=foo#qux", base);
  assertEquals(url.href, "https://foo:bar@baz.qat:8000/foo/bar?baz=foo#qux");
});

test(function urlBaseString(): void {
  const url = new URL(
    "/foo/bar?baz=foo#qux",
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assertEquals(url.href, "https://foo:bar@baz.qat:8000/foo/bar?baz=foo#qux");
});

test(function deletingAllParamsRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?param1&param2");
  url.searchParams.delete("param1");
  url.searchParams.delete("param2");
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});

test(function removingNonExistentParamRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?");
  assertEquals(url.href, "http://example.com/?");
  url.searchParams.delete("param1");
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});

test(function sortingNonExistentParamRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?");
  assertEquals(url.href, "http://example.com/?");
  url.searchParams.sort();
  assertEquals(url.href, "http://example.com/");
  assertEquals(url.search, "");
});
