// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

test(function urlParsing() {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assertEqual(url.hash, "#qat");
  assertEqual(url.host, "baz.qat:8000");
  assertEqual(url.hostname, "baz.qat");
  assertEqual(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assertEqual(url.origin, "https://baz.qat:8000");
  assertEqual(url.password, "bar");
  assertEqual(url.pathname, "/qux/quux");
  assertEqual(url.port, "8000");
  assertEqual(url.protocol, "https:");
  assertEqual(url.search, "?foo=bar&baz=12");
  assertEqual(url.searchParams.getAll("foo"), ["bar"]);
  assertEqual(url.searchParams.getAll("baz"), ["12"]);
  assertEqual(url.username, "foo");
  assertEqual(
    String(url),
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assertEqual(
    JSON.stringify({ key: url }),
    `{"key":"https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"}`
  );
});

test(function urlModifications() {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  url.hash = "";
  assertEqual(url.href, "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12");
  url.host = "qat.baz:8080";
  assertEqual(url.href, "https://foo:bar@qat.baz:8080/qux/quux?foo=bar&baz=12");
  url.hostname = "foo.bar";
  assertEqual(url.href, "https://foo:bar@foo.bar:8080/qux/quux?foo=bar&baz=12");
  url.password = "qux";
  assertEqual(url.href, "https://foo:qux@foo.bar:8080/qux/quux?foo=bar&baz=12");
  url.pathname = "/foo/bar%qat";
  assertEqual(
    url.href,
    "https://foo:qux@foo.bar:8080/foo/bar%qat?foo=bar&baz=12"
  );
  url.port = "";
  assertEqual(url.href, "https://foo:qux@foo.bar/foo/bar%qat?foo=bar&baz=12");
  url.protocol = "http:";
  assertEqual(url.href, "http://foo:qux@foo.bar/foo/bar%qat?foo=bar&baz=12");
  url.search = "?foo=bar&foo=baz";
  assertEqual(url.href, "http://foo:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz");
  assertEqual(url.searchParams.getAll("foo"), ["bar", "baz"]);
  url.username = "foo@bar";
  assertEqual(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz"
  );
  url.searchParams.set("bar", "qat");
  assertEqual(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz&bar=qat"
  );
  url.searchParams.delete("foo");
  assertEqual(url.href, "http://foo%40bar:qux@foo.bar/foo/bar%qat?bar=qat");
  url.searchParams.append("foo", "bar");
  assertEqual(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?bar=qat&foo=bar"
  );
});

test(function urlModifyHref() {
  const url = new URL("http://example.com/");
  url.href = "https://foo:bar@example.com:8080/baz/qat#qux";
  assertEqual(url.protocol, "https:");
  assertEqual(url.username, "foo");
  assertEqual(url.password, "bar");
  assertEqual(url.host, "example.com:8080");
  assertEqual(url.hostname, "example.com");
  assertEqual(url.pathname, "/baz/qat");
  assertEqual(url.hash, "#qux");
});

test(function urlModifyPathname() {
  const url = new URL("http://foo.bar/baz%qat/qux%quux");
  assertEqual(url.pathname, "/baz%qat/qux%quux");
  url.pathname = url.pathname;
  assertEqual(url.pathname, "/baz%qat/qux%quux");
  url.pathname = "baz#qat qux";
  assertEqual(url.pathname, "/baz%23qat%20qux");
  url.pathname = url.pathname;
  assertEqual(url.pathname, "/baz%23qat%20qux");
});

test(function urlModifyHash() {
  const url = new URL("http://foo.bar");
  url.hash = "%foo bar/qat%qux#bar";
  assertEqual(url.hash, "#%foo%20bar/qat%qux#bar");
  url.hash = url.hash;
  assertEqual(url.hash, "#%foo%20bar/qat%qux#bar");
});

test(function urlSearchParamsReuse() {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  const sp = url.searchParams;
  url.host = "baz.qat";
  assert(sp === url.searchParams, "Search params should be reused.");
});

test(function urlBaseURL() {
  const base = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  const url = new URL("/foo/bar?baz=foo#qux", base);
  assertEqual(url.href, "https://foo:bar@baz.qat:8000/foo/bar?baz=foo#qux");
});

test(function urlBaseString() {
  const url = new URL(
    "/foo/bar?baz=foo#qux",
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assertEqual(url.href, "https://foo:bar@baz.qat:8000/foo/bar?baz=foo#qux");
});
