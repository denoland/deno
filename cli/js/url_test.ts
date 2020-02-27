// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function urlParsing(): void {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assert.equals(url.hash, "#qat");
  assert.equals(url.host, "baz.qat:8000");
  assert.equals(url.hostname, "baz.qat");
  assert.equals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assert.equals(url.origin, "https://baz.qat:8000");
  assert.equals(url.password, "bar");
  assert.equals(url.pathname, "/qux/quux");
  assert.equals(url.port, "8000");
  assert.equals(url.protocol, "https:");
  assert.equals(url.search, "?foo=bar&baz=12");
  assert.equals(url.searchParams.getAll("foo"), ["bar"]);
  assert.equals(url.searchParams.getAll("baz"), ["12"]);
  assert.equals(url.username, "foo");
  assert.equals(
    String(url),
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assert.equals(
    JSON.stringify({ key: url }),
    `{"key":"https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"}`
  );
});

test(function urlModifications(): void {
  const url = new URL(
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  url.hash = "";
  assert.equals(
    url.href,
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12"
  );
  url.host = "qat.baz:8080";
  assert.equals(
    url.href,
    "https://foo:bar@qat.baz:8080/qux/quux?foo=bar&baz=12"
  );
  url.hostname = "foo.bar";
  assert.equals(
    url.href,
    "https://foo:bar@foo.bar:8080/qux/quux?foo=bar&baz=12"
  );
  url.password = "qux";
  assert.equals(
    url.href,
    "https://foo:qux@foo.bar:8080/qux/quux?foo=bar&baz=12"
  );
  url.pathname = "/foo/bar%qat";
  assert.equals(
    url.href,
    "https://foo:qux@foo.bar:8080/foo/bar%qat?foo=bar&baz=12"
  );
  url.port = "";
  assert.equals(url.href, "https://foo:qux@foo.bar/foo/bar%qat?foo=bar&baz=12");
  url.protocol = "http:";
  assert.equals(url.href, "http://foo:qux@foo.bar/foo/bar%qat?foo=bar&baz=12");
  url.search = "?foo=bar&foo=baz";
  assert.equals(url.href, "http://foo:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz");
  assert.equals(url.searchParams.getAll("foo"), ["bar", "baz"]);
  url.username = "foo@bar";
  assert.equals(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz"
  );
  url.searchParams.set("bar", "qat");
  assert.equals(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?foo=bar&foo=baz&bar=qat"
  );
  url.searchParams.delete("foo");
  assert.equals(url.href, "http://foo%40bar:qux@foo.bar/foo/bar%qat?bar=qat");
  url.searchParams.append("foo", "bar");
  assert.equals(
    url.href,
    "http://foo%40bar:qux@foo.bar/foo/bar%qat?bar=qat&foo=bar"
  );
});

test(function urlModifyHref(): void {
  const url = new URL("http://example.com/");
  url.href = "https://foo:bar@example.com:8080/baz/qat#qux";
  assert.equals(url.protocol, "https:");
  assert.equals(url.username, "foo");
  assert.equals(url.password, "bar");
  assert.equals(url.host, "example.com:8080");
  assert.equals(url.hostname, "example.com");
  assert.equals(url.pathname, "/baz/qat");
  assert.equals(url.hash, "#qux");
});

test(function urlModifyPathname(): void {
  const url = new URL("http://foo.bar/baz%qat/qux%quux");
  assert.equals(url.pathname, "/baz%qat/qux%quux");
  url.pathname = url.pathname;
  assert.equals(url.pathname, "/baz%qat/qux%quux");
  url.pathname = "baz#qat qux";
  assert.equals(url.pathname, "/baz%23qat%20qux");
  url.pathname = url.pathname;
  assert.equals(url.pathname, "/baz%23qat%20qux");
});

test(function urlModifyHash(): void {
  const url = new URL("http://foo.bar");
  url.hash = "%foo bar/qat%qux#bar";
  assert.equals(url.hash, "#%foo%20bar/qat%qux#bar");
  url.hash = url.hash;
  assert.equals(url.hash, "#%foo%20bar/qat%qux#bar");
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
  assert.equals(url.href, "https://foo:bar@baz.qat:8000/foo/bar?baz=foo#qux");
});

test(function urlBaseString(): void {
  const url = new URL(
    "/foo/bar?baz=foo#qux",
    "https://foo:bar@baz.qat:8000/qux/quux?foo=bar&baz=12#qat"
  );
  assert.equals(url.href, "https://foo:bar@baz.qat:8000/foo/bar?baz=foo#qux");
});

test(function urlRelativeWithBase(): void {
  assert.equals(new URL("", "file:///a/a/a").href, "file:///a/a/a");
  assert.equals(new URL(".", "file:///a/a/a").href, "file:///a/a/");
  assert.equals(new URL("..", "file:///a/a/a").href, "file:///a/");
  assert.equals(new URL("b", "file:///a/a/a").href, "file:///a/a/b");
  assert.equals(new URL("b", "file:///a/a/a/").href, "file:///a/a/a/b");
  assert.equals(new URL("b/", "file:///a/a/a").href, "file:///a/a/b/");
  assert.equals(new URL("../b", "file:///a/a/a").href, "file:///a/b");
});

test(function emptyBasePath(): void {
  assert.equals(new URL("", "http://example.com").href, "http://example.com/");
});

test(function deletingAllParamsRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?param1&param2");
  url.searchParams.delete("param1");
  url.searchParams.delete("param2");
  assert.equals(url.href, "http://example.com/");
  assert.equals(url.search, "");
});

test(function removingNonExistentParamRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?");
  assert.equals(url.href, "http://example.com/?");
  url.searchParams.delete("param1");
  assert.equals(url.href, "http://example.com/");
  assert.equals(url.search, "");
});

test(function sortingNonExistentParamRemovesQuestionMarkFromURL(): void {
  const url = new URL("http://example.com/?");
  assert.equals(url.href, "http://example.com/?");
  url.searchParams.sort();
  assert.equals(url.href, "http://example.com/");
  assert.equals(url.search, "");
});

/*
test(function customInspectFunction(): void {
  const url = new URL("http://example.com/?");
  assert.equals(
    Deno.inspect(url),
    'URL { href: "http://example.com/?", origin: "http://example.com", protocol: "http:", username: "", password: "", host: "example.com", hostname: "example.com", port: "", pathname: "/", hash: "", search: "?" }'
  );
});
*/

test(function protocolNotHttpOrFile() {
  const url = new URL("about:blank");
  assert.equals(url.href, "about:blank");
  assert.equals(url.protocol, "about:");
  assert.equals(url.origin, "null");
});

test(function createBadUrl(): void {
  assert.throws(() => {
    new URL("0.0.0.0:8080");
  });
});

if (import.meta.main) {
  Deno.runTests();
}
