import { assertEquals } from "../testing/asserts.ts";
import { parse, format, resolve } from "./url.ts";

const { test } = Deno;
const testUrl =
  "https://disizali:123456@deno.land:80/std/manual.md?future=deno#hash";

test("[node/url] parse url", function parseUrl() {
  const parsed = parse(testUrl);
  assertEquals(parsed.protocol, "https:");
  assertEquals(parsed.slashes, true);
  assertEquals(parsed.auth, "disizali:123456");
  assertEquals(parsed.host, "deno.land:80");
  assertEquals(parsed.hostname, "deno.land");
  assertEquals(parsed.hash, "#hash");
  assertEquals(parsed.search, "?future=deno");
  assertEquals(parsed.pathname, "/std/manual.md");
  assertEquals(parsed.path, "/std/manual.md?future=deno");
  assertEquals(parsed.href, testUrl);
});

test("[node/url] format url", function parseUrl() {
  assertEquals(testUrl, format(parse(testUrl)));
});

test("[node/url] resolve url", function parseUrl() {
  assertEquals(resolve("/one/two/three", "four"), "/one/two/four");
  assertEquals(resolve("http://deno.land/", "/std"), "http://deno.land/std");
  assertEquals(resolve("http://deno.land/std", "/x"), "http://deno.land/x");
});
