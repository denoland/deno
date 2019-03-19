// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

test(function booleanAndAliasIsNotUnknown() {
  const unknown = [];
  function unknownFn(arg): boolean {
    unknown.push(arg);
    return false;
  }
  const aliased = ["-h", "true", "--derp", "true"];
  const regular = ["--herp", "true", "-d", "true"];
  const opts = {
    alias: { h: "herp" },
    boolean: "h",
    unknown: unknownFn
  };
  parse(aliased, opts);
  parse(regular, opts);

  assertEquals(unknown, ["--derp", "-d"]);
});

test(function flagBooleanTrueAnyDoubleHyphenArgumentIsNotUnknown() {
  const unknown = [];
  function unknownFn(arg): boolean {
    unknown.push(arg);
    return false;
  }
  const argv = parse(["--honk", "--tacos=good", "cow", "-p", "55"], {
    boolean: true,
    unknown: unknownFn
  });
  assertEquals(unknown, ["--tacos=good", "cow", "-p"]);
  assertEquals(argv, {
    honk: true,
    _: []
  });
});

test(function stringAndAliasIsNotUnkown() {
  const unknown = [];
  function unknownFn(arg): boolean {
    unknown.push(arg);
    return false;
  }
  const aliased = ["-h", "hello", "--derp", "goodbye"];
  const regular = ["--herp", "hello", "-d", "moon"];
  const opts = {
    alias: { h: "herp" },
    string: "h",
    unknown: unknownFn
  };
  parse(aliased, opts);
  parse(regular, opts);

  assertEquals(unknown, ["--derp", "-d"]);
});

test(function defaultAndAliasIsNotUnknown() {
  const unknown = [];
  function unknownFn(arg): boolean {
    unknown.push(arg);
    return false;
  }
  const aliased = ["-h", "hello"];
  const regular = ["--herp", "hello"];
  const opts = {
    default: { h: "bar" },
    alias: { h: "herp" },
    unknown: unknownFn
  };
  parse(aliased, opts);
  parse(regular, opts);

  assertEquals(unknown, []);
});

test(function valueFollowingDoubleHyphenIsNotUnknown() {
  const unknown = [];
  function unknownFn(arg): boolean {
    unknown.push(arg);
    return false;
  }
  const aliased = ["--bad", "--", "good", "arg"];
  const opts = {
    "--": true,
    unknown: unknownFn
  };
  const argv = parse(aliased, opts);

  assertEquals(unknown, ["--bad"]);
  assertEquals(argv, {
    "--": ["good", "arg"],
    _: []
  });
});
