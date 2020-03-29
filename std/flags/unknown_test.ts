// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { parse } from "./mod.ts";

Deno.test(function booleanAndAliasIsNotUnknown(): void {
  const unknown: unknown[] = [];
  function unknownFn(arg: unknown): boolean {
    unknown.push(arg);
    return false;
  }
  const aliased = ["-h", "true", "--derp", "true"];
  const regular = ["--herp", "true", "-d", "true"];
  const opts = {
    alias: { h: "herp" },
    boolean: "h",
    unknown: unknownFn,
  };
  parse(aliased, opts);
  parse(regular, opts);

  assertEquals(unknown, ["--derp", "-d"]);
});

Deno.test(function flagBooleanTrueAnyDoubleHyphenArgumentIsNotUnknown(): void {
  const unknown: unknown[] = [];
  function unknownFn(arg: unknown): boolean {
    unknown.push(arg);
    return false;
  }
  const argv = parse(["--honk", "--tacos=good", "cow", "-p", "55"], {
    boolean: true,
    unknown: unknownFn,
  });
  assertEquals(unknown, ["--tacos=good", "cow", "-p"]);
  assertEquals(argv, {
    honk: true,
    _: [],
  });
});

Deno.test(function stringAndAliasIsNotUnkown(): void {
  const unknown: unknown[] = [];
  function unknownFn(arg: unknown): boolean {
    unknown.push(arg);
    return false;
  }
  const aliased = ["-h", "hello", "--derp", "goodbye"];
  const regular = ["--herp", "hello", "-d", "moon"];
  const opts = {
    alias: { h: "herp" },
    string: "h",
    unknown: unknownFn,
  };
  parse(aliased, opts);
  parse(regular, opts);

  assertEquals(unknown, ["--derp", "-d"]);
});

Deno.test(function defaultAndAliasIsNotUnknown(): void {
  const unknown: unknown[] = [];
  function unknownFn(arg: unknown): boolean {
    unknown.push(arg);
    return false;
  }
  const aliased = ["-h", "hello"];
  const regular = ["--herp", "hello"];
  const opts = {
    default: { h: "bar" },
    alias: { h: "herp" },
    unknown: unknownFn,
  };
  parse(aliased, opts);
  parse(regular, opts);

  assertEquals(unknown, []);
});

Deno.test(function valueFollowingDoubleHyphenIsNotUnknown(): void {
  const unknown: unknown[] = [];
  function unknownFn(arg: unknown): boolean {
    unknown.push(arg);
    return false;
  }
  const aliased = ["--bad", "--", "good", "arg"];
  const opts = {
    "--": true,
    unknown: unknownFn,
  };
  const argv = parse(aliased, opts);

  assertEquals(unknown, ["--bad"]);
  assertEquals(argv, {
    "--": ["good", "arg"],
    _: [],
  });
});
