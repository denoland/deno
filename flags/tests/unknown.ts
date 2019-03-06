// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function booleanAndAliasIsNotUnknown() {
  const unknown = [];
  function unknownFn(arg) {
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
  const aliasedArgv = parse(aliased, opts);
  const propertyArgv = parse(regular, opts);

  assertEq(unknown, ["--derp", "-d"]);
});

test(function flagBooleanTrueAnyDoubleHyphenArgumentIsNotUnknown() {
  const unknown = [];
  function unknownFn(arg) {
    unknown.push(arg);
    return false;
  }
  const argv = parse(["--honk", "--tacos=good", "cow", "-p", "55"], {
    boolean: true,
    unknown: unknownFn
  });
  assertEq(unknown, ["--tacos=good", "cow", "-p"]);
  assertEq(argv, {
    honk: true,
    _: []
  });
});

test(function stringAndAliasIsNotUnkown() {
  const unknown = [];
  function unknownFn(arg) {
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
  const aliasedArgv = parse(aliased, opts);
  const propertyArgv = parse(regular, opts);

  assertEq(unknown, ["--derp", "-d"]);
});

test(function defaultAndAliasIsNotUnknown() {
  const unknown = [];
  function unknownFn(arg) {
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
  const aliasedArgv = parse(aliased, opts);
  const propertyArgv = parse(regular, opts);

  assertEq(unknown, []);
});

test(function valueFollowingDoubleHyphenIsNotUnknown() {
  const unknown = [];
  function unknownFn(arg) {
    unknown.push(arg);
    return false;
  }
  const aliased = ["--bad", "--", "good", "arg"];
  const opts = {
    "--": true,
    unknown: unknownFn
  };
  const argv = parse(aliased, opts);

  assertEq(unknown, ["--bad"]);
  assertEq(argv, {
    "--": ["good", "arg"],
    _: []
  });
});
