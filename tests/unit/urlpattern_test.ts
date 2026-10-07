// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals, assertThrows } from "./test_util.ts";
import { assertType, IsExact } from "@std/testing/types";

Deno.test(function urlPatternFromString() {
  const pattern = new URLPattern("https://deno.land/foo/:bar");
  assertEquals(pattern.protocol, "https");
  assertEquals(pattern.hostname, "deno.land");
  assertEquals(pattern.pathname, "/foo/:bar");

  assert(pattern.test("https://deno.land/foo/x"));
  assert(!pattern.test("https://deno.com/foo/x"));
  const match = pattern.exec("https://deno.land/foo/x");
  assert(match);
  assertEquals(match.pathname.input, "/foo/x");
  assertEquals(match.pathname.groups, { bar: "x" });

  // group values should be nullable
  const val = match.pathname.groups.val;
  assertType<IsExact<typeof val, string | undefined>>(true);
});

Deno.test(function urlPatternFromStringWithBase() {
  const pattern = new URLPattern("/foo/:bar", "https://deno.land");
  assertEquals(pattern.protocol, "https");
  assertEquals(pattern.hostname, "deno.land");
  assertEquals(pattern.pathname, "/foo/:bar");

  assert(pattern.test("https://deno.land/foo/x"));
  assert(!pattern.test("https://deno.com/foo/x"));
  const match = pattern.exec("https://deno.land/foo/x");
  assert(match);
  assertEquals(match.pathname.input, "/foo/x");
  assertEquals(match.pathname.groups, { bar: "x" });
});

Deno.test(function urlPatternFromInit() {
  const pattern = new URLPattern({
    pathname: "/foo/:bar",
  });
  assertEquals(pattern.protocol, "*");
  assertEquals(pattern.hostname, "*");
  assertEquals(pattern.pathname, "/foo/:bar");

  assert(pattern.test("https://deno.land/foo/x"));
  assert(pattern.test("https://deno.com/foo/x"));
  assert(!pattern.test("https://deno.com/bar/x"));

  assert(pattern.test({ pathname: "/foo/x" }));
});

Deno.test(function urlPatternNamedGroupMatchingWholeComponent() {
  // https://github.com/denoland/deno/issues/28247
  const pattern = new URLPattern({ hostname: ":domain(.*)", pathname: "/foo" });
  // A failed match leaves a partially filled result object that the next
  // exec() call reuses.
  assertEquals(
    pattern.exec({ hostname: "example.com", pathname: "/bar" }),
    null,
  );

  const match = pattern.exec({ hostname: "localhost", pathname: "/foo" });
  assert(match);
  assertEquals(match.hostname.groups, { domain: "localhost" });
  // Unnamed wildcard groups keep the "0" key.
  assertEquals(match.search.groups, { "0": "" });
});

Deno.test(function urlPatternNamedGroupsMatchingWholeComponentsFromString() {
  // https://github.com/denoland/deno/issues/28247
  const pattern = new URLPattern(
    ":protocol://:domain(.*)::port?/:locale(de)?/:path(.*)?",
  );
  const match = pattern.exec("https://localhost:5173/de");
  assert(match);
  assertEquals(match.protocol.groups, { protocol: "https" });
  assertEquals(match.hostname.groups, { domain: "localhost" });
  assertEquals(match.port.groups, { port: "5173" });
  assertEquals(match.pathname.groups.locale, "de");
});

Deno.test(function urlPatternWithPrototypePollution() {
  const originalExec = RegExp.prototype.exec;
  try {
    RegExp.prototype.exec = () => {
      throw Error();
    };
    const pattern = new URLPattern({
      pathname: "/foo/:bar",
    });
    assert(pattern.test("https://deno.land/foo/x"));
  } finally {
    RegExp.prototype.exec = originalExec;
  }
});

Deno.test(function urlPatternFlagsRegression() {
  new URLPattern({ pathname: "/install(\.sh|\.ps1)" });
});

Deno.test(function urlPatternIgnoreCase() {
  const p = new URLPattern({ pathname: "/test" }, { ignoreCase: true });
  assert(p.test("/test", "http://localhost"));
  assert(p.test("/TeSt", "http://localhost"));
});

Deno.test(function urlPatternInvalidNameError() {
  // A ":" not followed by a valid group name (e.g. file-router syntax such as
  // `[:slug]` expanding to `::slug`) should produce an actionable error that
  // echoes the offending component, points a caret at it, and includes a hint.
  const err = assertThrows(
    () => new URLPattern({ pathname: "/::slug" }),
    TypeError,
  ) as TypeError;
  assert(
    err.message.includes('Failed to parse pathname from "/::slug"'),
    `missing component context: ${err.message}`,
  );
  assert(err.message.includes("^"), `missing caret: ${err.message}`);
  assert(
    err.message.includes('hint: ":" starts a named group'),
    `missing hint: ${err.message}`,
  );
});

Deno.test(function urlPatternInvalidNameErrorFromString() {
  // For a constructor string we cannot reliably map the position back to the
  // full input, but the input and hint should still be surfaced.
  const err = assertThrows(
    () => new URLPattern("https://example.com/::slug"),
    TypeError,
  ) as TypeError;
  assert(
    err.message.includes(
      'Failed to parse URLPattern from "https://example.com/::slug"',
    ),
    `missing input context: ${err.message}`,
  );
  assert(
    err.message.includes('hint: ":" starts a named group'),
    `missing hint: ${err.message}`,
  );
});
