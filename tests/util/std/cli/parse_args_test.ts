// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { Args, parseArgs, ParseOptions } from "./parse_args.ts";
import { assertType, IsExact } from "../testing/types.ts";

// flag boolean true (default all --args to boolean)
Deno.test("flagBooleanTrue", function () {
  const argv = parseArgs(["moo", "--honk", "cow"], {
    boolean: true,
  });

  assertEquals(argv, {
    honk: true,
    _: ["moo", "cow"],
  });

  assertEquals(typeof argv.honk, "boolean");
});

// flag boolean true only affects double hyphen arguments without equals signs
Deno.test("flagBooleanTrueOnlyAffectsDoubleDash", function () {
  const argv = parseArgs(["moo", "--honk", "cow", "-p", "55", "--tacos=good"], {
    boolean: true,
  });

  assertEquals(argv, {
    honk: true,
    tacos: "good",
    p: 55,
    _: ["moo", "cow"],
  });

  assertEquals(typeof argv.honk, "boolean");
});

Deno.test("flagBooleanDefaultFalse", function () {
  const argv = parseArgs(["moo"], {
    boolean: ["t", "verbose"],
    default: { verbose: false, t: false },
  });

  assertEquals(argv, {
    verbose: false,
    t: false,
    _: ["moo"],
  });

  assertEquals(typeof argv.verbose, "boolean");
  assertEquals(typeof argv.t, "boolean");
});

Deno.test("booleanGroups", function () {
  const argv = parseArgs(["-x", "-z", "one", "two", "three"], {
    boolean: ["x", "y", "z"],
  });

  assertEquals(argv, {
    x: true,
    y: false,
    z: true,
    _: ["one", "two", "three"],
  });

  assertEquals(typeof argv.x, "boolean");
  assertEquals(typeof argv.y, "boolean");
  assertEquals(typeof argv.z, "boolean");
});

Deno.test("booleanAndAliasDefaultsToFalseWithNoArgs", function (): void {
  const argv = parseArgs([], {
    string: ["foo"],
    boolean: ["bar"],
    alias: {
      bar: "b",
    },
  });

  assertEquals(argv, {
    bar: false,
    b: false,
    _: [],
  });
});

Deno.test("booleanAndAliasWithChainableApi", function () {
  const aliased = ["-h", "derp"];
  const regular = ["--herp", "derp"];
  const aliasedArgv = parseArgs(aliased, {
    boolean: "herp",
    alias: { h: "herp" },
  });
  const propertyArgv = parseArgs(regular, {
    boolean: "herp",
    alias: { h: "herp" },
  });
  const expected = {
    herp: true,
    h: true,
    _: ["derp"],
  };

  assertEquals(aliasedArgv, expected);
  assertEquals(propertyArgv, expected);
});

Deno.test("booleanAndAliasWithOptionsHash", function () {
  const aliased = ["-h", "derp"];
  const regular = ["--herp", "derp"];
  const opts = {
    alias: { h: "herp" },
    boolean: "herp",
  } as const;
  const aliasedArgv = parseArgs(aliased, opts);
  const propertyArgv = parseArgs(regular, opts);
  const expected = {
    herp: true,
    h: true,
    _: ["derp"],
  };
  assertEquals(aliasedArgv, expected);
  assertEquals(propertyArgv, expected);
});

Deno.test("booleanAndAliasArrayWithOptionsHash", function () {
  const aliased = ["-h", "derp"];
  const regular = ["--herp", "derp"];
  const alt = ["--harp", "derp"];
  const opts = {
    alias: { h: ["herp", "harp"] },
    boolean: "h",
  } as const;
  const aliasedArgv = parseArgs(aliased, opts);
  const propertyArgv = parseArgs(regular, opts);
  const altPropertyArgv = parseArgs(alt, opts);
  const expected = {
    harp: true,
    herp: true,
    h: true,
    _: ["derp"],
  };
  assertEquals(aliasedArgv, expected);
  assertEquals(propertyArgv, expected);
  assertEquals(altPropertyArgv, expected);
});

Deno.test("booleanAndAliasUsingExplicitTrue", function () {
  const aliased = ["-h", "true"];
  const regular = ["--herp", "true"];
  const opts = {
    alias: { h: "herp" },
    boolean: "h",
  } as const;
  const aliasedArgv = parseArgs(aliased, opts);
  const propertyArgv = parseArgs(regular, opts);
  const expected = {
    herp: true,
    h: true,
    _: [],
  };

  assertEquals(aliasedArgv, expected);
  assertEquals(propertyArgv, expected);
});

// regression, see https://github.com/substack/node-optimist/issues/71
// boolean and --x=true
Deno.test("booleanAndNonBoolean", function () {
  const parsed = parseArgs(["--boool", "--other=true"], {
    boolean: "boool",
  });

  assertEquals(parsed.boool, true);
  assertEquals(parsed.other, "true");

  const parsed2 = parseArgs(["--boool", "--other=false"], {
    boolean: "boool",
  });

  assertEquals(parsed2.boool, true);
  assertEquals(parsed2.other, "false");
});

Deno.test("booleanParsingTrue", function () {
  const parsed = parseArgs(["--boool=true"], {
    default: {
      boool: false,
    },
    boolean: ["boool"],
  });

  assertEquals(parsed.boool, true);
});

Deno.test("booleanParsingFalse", function () {
  const parsed = parseArgs(["--boool=false"], {
    default: {
      boool: true,
    },
    boolean: ["boool"],
  });

  assertEquals(parsed.boool, false);
});

Deno.test("booleanParsingTrueLike", function () {
  const parsed = parseArgs(["-t", "true123"], { boolean: ["t"] });
  assertEquals(parsed.t, true);

  const parsed2 = parseArgs(["-t", "123"], { boolean: ["t"] });
  assertEquals(parsed2.t, true);

  const parsed3 = parseArgs(["-t", "false123"], { boolean: ["t"] });
  assertEquals(parsed3.t, true);
});

Deno.test("booleanNegationAfterBoolean", function () {
  const parsed = parseArgs(["--foo", "--no-foo"], {
    boolean: ["foo"],
    negatable: ["foo"],
  });
  assertEquals(parsed.foo, false);

  const parsed2 = parseArgs(["--foo", "--no-foo", "123"], {
    boolean: ["foo"],
    negatable: ["foo"],
  });
  assertEquals(parsed2.foo, false);
});

Deno.test("booleanAfterBooleanNegation", function () {
  const parsed = parseArgs(["--no-foo", "--foo"], {
    boolean: ["foo"],
    negatable: ["foo"],
  });
  assertEquals(parsed.foo, true);

  const parsed2 = parseArgs(["--no-foo", "--foo", "123"], {
    boolean: ["foo"],
    negatable: ["foo"],
  });
  assertEquals(parsed2.foo, true);
});

Deno.test("latestFlagIsBooleanNegation", function () {
  const parsed = parseArgs(["--no-foo", "--foo", "--no-foo"], {
    boolean: ["foo"],
    negatable: ["foo"],
  });
  assertEquals(parsed.foo, false);

  const parsed2 = parseArgs(["--no-foo", "--foo", "--no-foo", "123"], {
    boolean: ["foo"],
    negatable: ["foo"],
  });
  assertEquals(parsed2.foo, false);
});

Deno.test("latestFlagIsBoolean", function () {
  const parsed = parseArgs(["--foo", "--no-foo", "--foo"], {
    boolean: ["foo"],
    negatable: ["foo"],
  });
  assertEquals(parsed.foo, true);

  const parsed2 = parseArgs(["--foo", "--no-foo", "--foo", "123"], {
    boolean: ["foo"],
    negatable: ["foo"],
  });
  assertEquals(parsed2.foo, true);
});

Deno.test("hyphen", function () {
  assertEquals(parseArgs(["-n", "-"]), { n: "-", _: [] });
  assertEquals(parseArgs(["-"]), { _: ["-"] });
  assertEquals(parseArgs(["-f-"]), { f: "-", _: [] });
  assertEquals(parseArgs(["-b", "-"], { boolean: "b" }), { b: true, _: ["-"] });
  assertEquals(parseArgs(["-s", "-"], { string: "s" }), { s: "-", _: [] });
});

Deno.test("doubleDash", function () {
  assertEquals(parseArgs(["-a", "--", "b"]), { a: true, _: ["b"] });
  assertEquals(parseArgs(["--a", "--", "b"]), { a: true, _: ["b"] });
  assertEquals(parseArgs(["--a", "--", "b"]), { a: true, _: ["b"] });
});

Deno.test("moveArgsAfterDoubleDashIntoOwnArray", function () {
  assertEquals(
    parseArgs(["--name", "John", "before", "--", "after"], { "--": true }),
    {
      name: "John",
      _: ["before"],
      "--": ["after"],
    },
  );
});

Deno.test("booleanDefaultTrue", function () {
  const argv = parseArgs([], {
    boolean: "sometrue",
    default: { sometrue: true },
  });
  assertEquals(argv.sometrue, true);
});

Deno.test("booleanDefaultFalse", function () {
  const argv = parseArgs([], {
    boolean: "somefalse",
    default: { somefalse: false },
  });
  assertEquals(argv.somefalse, false);
});

Deno.test("booleanDefaultNull", function () {
  const argv = parseArgs([], {
    boolean: "maybe",
    default: { maybe: null },
  });
  assertEquals(argv.maybe, null);
  const argv2 = parseArgs(["--maybe"], {
    boolean: "maybe",
    default: { maybe: null },
  });
  assertEquals(argv2.maybe, true);
});

Deno.test("dottedAlias", function () {
  const argv = parseArgs(["--a.b", "22"], {
    default: { "a.b": 11 },
    alias: { "a.b": "aa.bb" },
  });
  assertEquals(argv.a.b, 22);
  assertEquals(argv.aa.bb, 22);
});

Deno.test("dottedDefault", function () {
  const argv = parseArgs([], {
    default: { "a.b": 11 },
    alias: { "a.b": "aa.bb" },
  });
  assertEquals(argv.a.b, 11);
  assertEquals(argv.aa.bb, 11);
});

Deno.test("dottedDefaultWithNoAlias", function () {
  const argv = parseArgs([], { default: { "a.b": 11 } });
  assertEquals(argv.a.b, 11);
});

Deno.test("short", function () {
  const argv = parseArgs(["-b=123"]);
  assertEquals(argv, { b: 123, _: [] });
});

Deno.test("multiShort", function () {
  const argv = parseArgs(["-a=whatever", "-b=robots"]);
  assertEquals(argv, { a: "whatever", b: "robots", _: [] });
});

Deno.test("longOpts", function () {
  assertEquals(parseArgs(["--bool"]), { bool: true, _: [] });
  assertEquals(parseArgs(["--pow", "xixxle"]), { pow: "xixxle", _: [] });
  assertEquals(parseArgs(["--pow=xixxle"]), { pow: "xixxle", _: [] });
  assertEquals(parseArgs(["--host", "localhost", "--port", "555"]), {
    host: "localhost",
    port: 555,
    _: [],
  });
  assertEquals(parseArgs(["--host=localhost", "--port=555"]), {
    host: "localhost",
    port: 555,
    _: [],
  });
});

Deno.test("nums", function () {
  const argv = parseArgs([
    "-x",
    "1234",
    "-y",
    "5.67",
    "-z",
    "1e7",
    "-w",
    "10f",
    "--hex",
    "0xdeadbeef",
    "789",
  ]);
  assertEquals(argv, {
    x: 1234,
    y: 5.67,
    z: 1e7,
    w: "10f",
    hex: 0xdeadbeef,
    _: [789],
  });
  assertEquals(typeof argv.x, "number");
  assertEquals(typeof argv.y, "number");
  assertEquals(typeof argv.z, "number");
  assertEquals(typeof argv.w, "string");
  assertEquals(typeof argv.hex, "number");
  assertEquals(typeof argv._[0], "number");
});

Deno.test("alreadyNumber", function () {
  const argv = parseArgs(["-x", "1234", "789"]);
  assertEquals(argv, { x: 1234, _: [789] });
  assertEquals(typeof argv.x, "number");
  assertEquals(typeof argv._[0], "number");
});

Deno.test("parseArgs", function () {
  assertEquals(parseArgs(["--no-moo"]), { "no-moo": true, _: [] });
  assertEquals(parseArgs(["-v", "a", "-v", "b", "-v", "c"]), {
    v: "c",
    _: [],
  });
});

Deno.test("comprehensive", function () {
  assertEquals(
    parseArgs([
      "--name=meowmers",
      "bare",
      "-cats",
      "woo",
      "-h",
      "awesome",
      "--multi=quux",
      "--key",
      "value",
      "-b",
      "--bool",
      "--no-meep",
      "--multi=baz",
      "-f=abc=def",
      "--foo=---=\\n--+34-=/=",
      "-e==",
      "--",
      "--not-a-flag",
      "eek",
    ]),
    {
      c: true,
      a: true,
      t: true,
      e: "=",
      f: "abc=def",
      foo: "---=\\n--+34-=/=",
      s: "woo",
      h: "awesome",
      b: true,
      bool: true,
      key: "value",
      multi: "baz",
      "no-meep": true,
      name: "meowmers",
      _: ["bare", "--not-a-flag", "eek"],
    },
  );
});

Deno.test("flagBoolean", function () {
  const argv = parseArgs(["-t", "moo"], { boolean: "t" });
  assertEquals(argv, { t: true, _: ["moo"] });
  assertEquals(typeof argv.t, "boolean");
});

Deno.test("flagBooleanValue", function () {
  const argv = parseArgs(["--verbose", "false", "moo", "-t", "true"], {
    boolean: ["t", "verbose"],
    default: { verbose: true },
  });

  assertEquals(argv, {
    verbose: false,
    t: true,
    _: ["moo"],
  });

  assertEquals(typeof argv.verbose, "boolean");
  assertEquals(typeof argv.t, "boolean");
});

Deno.test("newlinesInParams", function () {
  const args = parseArgs(["-s", "X\nX"]);
  assertEquals(args, { _: [], s: "X\nX" });

  // reproduce in bash:
  // VALUE="new
  // line"
  // deno program.js --s="$VALUE"
  const args2 = parseArgs(["--s=X\nX"]);
  assertEquals(args2, { _: [], s: "X\nX" });
});

Deno.test("strings", function () {
  const s = parseArgs(["-s", "0001234"], { string: "s" }).s;
  assertEquals(s, "0001234");
  assertEquals(typeof s, "string");

  const x = parseArgs(["-x", "56"], { string: "x" }).x;
  assertEquals(x, "56");
  assertEquals(typeof x, "string");
});

Deno.test("stringArgs", function () {
  const s = parseArgs(["  ", "  "], { string: "_" })._;
  assertEquals(s.length, 2);
  assertEquals(typeof s[0], "string");
  assertEquals(s[0], "  ");
  assertEquals(typeof s[1], "string");
  assertEquals(s[1], "  ");
});

Deno.test("emptyStrings", function () {
  const s = parseArgs(["-s"], { string: "s" }).s;
  assertEquals(s, "");
  assertEquals(typeof s, "string");

  const str = parseArgs(["--str"], { string: "str" }).str;
  assertEquals(str, "");
  assertEquals(typeof str, "string");

  const letters = parseArgs(["-art"], {
    string: ["a", "t"],
  });

  assertEquals(letters.a, "");
  assertEquals(letters.r, true);
  assertEquals(letters.t, "");
});

Deno.test("stringAndAlias", function () {
  const x = parseArgs(["--str", "000123"], {
    string: "s",
    alias: { s: "str" },
  });

  assertEquals(x.str, "000123");
  assertEquals(typeof x.str, "string");
  assertEquals(x.s, "000123");
  assertEquals(typeof x.s, "string");

  const y = parseArgs(["-s", "000123"], {
    string: "str",
    alias: { str: "s" },
  });

  assertEquals(y.str, "000123");
  assertEquals(typeof y.str, "string");
  assertEquals(y.s, "000123");
  assertEquals(typeof y.s, "string");
});

Deno.test("slashBreak", function () {
  assertEquals(parseArgs(["-I/foo/bar/baz"]), { I: "/foo/bar/baz", _: [] });
  assertEquals(parseArgs(["-xyz/foo/bar/baz"]), {
    x: true,
    y: true,
    z: "/foo/bar/baz",
    _: [],
  });
});

Deno.test("alias", function () {
  const argv = parseArgs(["-f", "11", "--zoom", "55"], {
    alias: { z: "zoom" },
  });
  assertEquals(argv.zoom, 55);
  assertEquals(argv.z, argv.zoom);
  assertEquals(argv.f, 11);
});

Deno.test("multiAlias", function () {
  const argv = parseArgs(["-f", "11", "--zoom", "55"], {
    alias: { z: ["zm", "zoom"] },
  });
  assertEquals(argv.zoom, 55);
  assertEquals(argv.z, argv.zoom);
  assertEquals(argv.z, argv.zm);
  assertEquals(argv.f, 11);
});

Deno.test("nestedDottedObjects", function () {
  const argv = parseArgs([
    "--foo.bar",
    "3",
    "--foo.baz",
    "4",
    "--foo.quux.quibble",
    "5",
    "--foo.quux.oO",
    "--beep.boop",
  ]);

  assertEquals(argv.foo, {
    bar: 3,
    baz: 4,
    quux: {
      quibble: 5,
      oO: true,
    },
  });
  assertEquals(argv.beep, { boop: true });
});

Deno.test("flagBuiltinProperty", function () {
  const argv = parseArgs(["--toString", "--valueOf", "foo"]);
  assertEquals(argv, { toString: true, valueOf: "foo", _: [] });
  assertEquals(typeof argv.toString, "boolean");
  assertEquals(typeof argv.valueOf, "string");
});

Deno.test("numericShortArgs", function () {
  assertEquals(parseArgs(["-n123"]), { n: 123, _: [] });
  assertEquals(parseArgs(["-123", "456"]), { 1: true, 2: true, 3: 456, _: [] });
});

Deno.test("short", function () {
  assertEquals(parseArgs(["-b"]), { b: true, _: [] });
  assertEquals(parseArgs(["foo", "bar", "baz"]), { _: ["foo", "bar", "baz"] });
  assertEquals(parseArgs(["-cats"]), {
    c: true,
    a: true,
    t: true,
    s: true,
    _: [],
  });
  assertEquals(parseArgs(["-cats", "meow"]), {
    c: true,
    a: true,
    t: true,
    s: "meow",
    _: [],
  });
  assertEquals(parseArgs(["-h", "localhost"]), { h: "localhost", _: [] });
  assertEquals(parseArgs(["-h", "localhost", "-p", "555"]), {
    h: "localhost",
    p: 555,
    _: [],
  });
});

Deno.test("mixedShortBoolAndCapture", function () {
  assertEquals(parseArgs(["-h", "localhost", "-fp", "555", "script.js"]), {
    f: true,
    p: 555,
    h: "localhost",
    _: ["script.js"],
  });
});

Deno.test("shortAndLong", function () {
  assertEquals(parseArgs(["-h", "localhost", "-fp", "555", "script.js"]), {
    f: true,
    p: 555,
    h: "localhost",
    _: ["script.js"],
  });
});

// stops parsing on the first non-option when stopEarly is set
Deno.test("stopParsing", function () {
  const argv = parseArgs(["--aaa", "bbb", "ccc", "--ddd"], {
    stopEarly: true,
  });

  assertEquals(argv, {
    aaa: "bbb",
    _: ["ccc", "--ddd"],
  });
});

Deno.test("booleanAndAliasIsNotUnknown", function () {
  const unknown: unknown[] = [];
  function unknownFn(arg: string, k?: string, v?: unknown): boolean {
    unknown.push({ arg, k, v });
    return false;
  }
  const aliased = ["-h", "true", "--derp", "true"];
  const regular = ["--herp", "true", "-d", "false"];
  const opts = {
    alias: { h: "herp" },
    boolean: "h",
    unknown: unknownFn,
  };
  parseArgs(aliased, opts);
  parseArgs(regular, opts);

  assertEquals(unknown, [
    { arg: "--derp", k: "derp", v: "true" },
    { arg: "-d", k: "d", v: "false" },
  ]);
});

Deno.test(
  "flagBooleanTrueAnyDoubleHyphenArgumentIsNotUnknown",
  function () {
    const unknown: unknown[] = [];
    function unknownFn(arg: string, k?: string, v?: unknown): boolean {
      unknown.push({ arg, k, v });
      return false;
    }
    const argv = parseArgs(["--honk", "--tacos=good", "cow", "-p", "55"], {
      boolean: true,
      unknown: unknownFn,
    });
    assertEquals(unknown, [
      { arg: "--tacos=good", k: "tacos", v: "good" },
      { arg: "cow", k: undefined, v: undefined },
      { arg: "-p", k: "p", v: "55" },
    ]);
    assertEquals(argv, {
      honk: true,
      _: [],
    });
  },
);

Deno.test("stringAndAliasIsNotUnknown", function () {
  const unknown: unknown[] = [];
  function unknownFn(arg: string, k?: string, v?: unknown): boolean {
    unknown.push({ arg, k, v });
    return false;
  }
  const aliased = ["-h", "hello", "--derp", "goodbye"];
  const regular = ["--herp", "hello", "-d", "moon"];
  const opts = {
    alias: { h: "herp" },
    string: "h",
    unknown: unknownFn,
  };
  parseArgs(aliased, opts);
  parseArgs(regular, opts);

  assertEquals(unknown, [
    { arg: "--derp", k: "derp", v: "goodbye" },
    { arg: "-d", k: "d", v: "moon" },
  ]);
});

Deno.test("defaultAndAliasIsNotUnknown", function () {
  const unknown: unknown[] = [];
  function unknownFn(arg: string, k?: string, v?: unknown): boolean {
    unknown.push({ arg, k, v });
    return false;
  }
  const aliased = ["-h", "hello"];
  const regular = ["--herp", "hello"];
  const opts = {
    default: { h: "bar" },
    alias: { h: "herp" },
    unknown: unknownFn,
  };
  parseArgs(aliased, opts);
  parseArgs(regular, opts);

  assertEquals(unknown, []);
});

Deno.test("valueFollowingDoubleHyphenIsNotUnknown", function () {
  const unknown: unknown[] = [];
  function unknownFn(arg: string, k?: string, v?: unknown): boolean {
    unknown.push({ arg, k, v });
    return false;
  }
  const aliased = ["--bad", "--", "good", "arg"];
  const opts = {
    "--": true,
    unknown: unknownFn,
  };
  const argv = parseArgs(aliased, opts);

  assertEquals(unknown, [{ arg: "--bad", k: "bad", v: true }]);
  assertEquals(argv, {
    "--": ["good", "arg"],
    _: [],
  });
});

Deno.test("whitespaceShouldBeWhitespace", function () {
  assertEquals(parseArgs(["-x", "\t"]).x, "\t");
});

Deno.test("collectArgsDefaultBehaviour", function () {
  const argv = parseArgs([
    "--foo",
    "bar",
    "--foo",
    "baz",
    "--beep",
    "boop",
    "--bool",
    "--bool",
  ]);

  assertEquals(argv, {
    foo: "baz",
    beep: "boop",
    bool: true,
    _: [],
  });
});

Deno.test("collectArgsWithDefaultBehaviour", function () {
  const argv = parseArgs([], {
    collect: ["foo"],
    default: {
      foo: ["bar", "baz"],
    },
  });

  assertEquals(argv, {
    foo: ["bar", "baz"],
    _: [],
  });
});

Deno.test("collectUnknownArgs", function () {
  const argv = parseArgs([
    "--foo",
    "bar",
    "--foo",
    "baz",
    "--beep",
    "boop",
    "--bib",
    "--bib",
    "--bab",
    "--bab",
  ], {
    collect: ["beep", "bib"],
  });

  assertEquals(argv, {
    foo: "baz",
    beep: ["boop"],
    bib: [true, true],
    bab: true,
    _: [],
  });
});

Deno.test("collectArgs", function () {
  const argv = parseArgs([
    "--bool",
    "--bool",
    "--boolArr",
    "--str",
    "foo",
    "--strArr",
    "beep",
    "--unknown",
    "--unknownArr",
  ], {
    boolean: ["bool", "boolArr"],
    string: ["str", "strArr"],
    collect: ["boolArr", "strArr", "unknownArr"],
    alias: {
      bool: "b",
      strArr: "S",
      boolArr: "B",
    },
  });

  assertEquals(argv, {
    bool: true,
    b: true,
    boolArr: [true],
    B: [true],
    str: "foo",
    strArr: ["beep"],
    S: ["beep"],
    unknown: true,
    unknownArr: [true],
    _: [],
  });
});

Deno.test("collectNegateableArgs", function () {
  const argv = parseArgs([
    "--foo",
    "123",
    "-f",
    "456",
    "--no-foo",
  ], {
    string: ["foo"],
    collect: ["foo"],
    negatable: ["foo"],
    alias: {
      foo: "f",
    },
  });

  assertEquals(argv, {
    foo: false,
    f: false,
    _: [],
  });
});

/** ---------------------- TYPE TESTS ---------------------- */

Deno.test("typesOfDefaultOptions", function () {
  const argv = parseArgs([]);
  assertType<
    IsExact<
      typeof argv,
      // deno-lint-ignore no-explicit-any
      & { [x: string]: any }
      & {
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAllBooleanDisabled", function () {
  const argv = parseArgs([], {
    boolean: false,
  });
  assertType<
    IsExact<
      typeof argv,
      // deno-lint-ignore no-explicit-any
      & { [x: string]: any }
      & {
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAllBooleanDisabledWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: false,
    default: {
      bar: 123,
    },
  });
  assertType<
    IsExact<
      typeof argv,
      // deno-lint-ignore no-explicit-any
      & { [x: string]: any }
      & {
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAllBooleanDisabledAndStringArgs", function () {
  const argv = parseArgs([], {
    boolean: false,
    string: ["foo"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo?: string | undefined;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAllBooleanDisabledAndStringArgsWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: false,
    string: ["foo"],
    default: {
      foo: 123,
      bar: false,
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: string | number;
        bar: unknown;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAllBoolean", function () {
  const argv = parseArgs([], {
    boolean: true,
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAllBooleanWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: true,
    default: {
      foo: "123",
      bar: 123,
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: unknown;
        bar: unknown;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAllBooleanAndStringArgs", function () {
  const argv = parseArgs([], {
    boolean: true,
    string: ["foo", "bar", "foo-bar"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo?: string | undefined;
        bar?: string | undefined;
        "foo-bar"?: string | undefined;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAllBooleanAndStringArgsWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: true,
    string: ["foo", "bar", "foo-bar"],
    default: {
      bar: 123,
      baz: new Date(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo?: string | undefined;
        bar: string | number;
        baz: unknown;
        "foo-bar"?: string | undefined;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfBooleanArgs", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "bar", "foo-bar"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: boolean;
        bar: boolean;
        "foo-bar": boolean;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfBooleanArgsWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "bar", "foo-bar"],
    default: {
      bar: 123,
      baz: "123",
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: boolean;
        bar: number | boolean;
        baz: unknown;
        "foo-bar": boolean;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfStringArgs", function () {
  const argv = parseArgs([], {
    string: ["foo", "bar", "foo-bar"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo?: string | undefined;
        bar?: string | undefined;
        "foo-bar"?: string | undefined;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfStringArgsWithDefaults", function () {
  const argv = parseArgs([], {
    string: ["foo", "bar", "foo-bar"],
    default: {
      bar: true,
      baz: 123,
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo?: string | undefined;
        bar: string | boolean;
        baz: unknown;
        "foo-bar"?: string | undefined;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfBooleanAndStringArgs", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "bar", "foo-bar"],
    string: ["beep", "boop", "beep-boop"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        beep?: string | undefined;
        boop?: string | undefined;
        "beep-boop"?: string | undefined;
        foo: boolean;
        bar: boolean;
        "foo-bar": boolean;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfBooleanAndStringArgsWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "bar", "foo-bar"],
    string: ["beep", "boop", "beep-boop"],
    default: {
      bar: 123,
      baz: new Error(),
      beep: new Date(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: boolean;
        boop?: string | undefined;
        "beep-boop"?: string | undefined;
        bar: number | boolean;
        baz: unknown;
        beep: string | Date;
        "foo-bar": boolean;
        _: Array<string | number>;
      }
    >
  >(true);
});

/** ------------------------ DOTTED OPTIONS ------------------------ */

Deno.test("typesOfDottedBooleanArgs", function () {
  const argv = parseArgs([], {
    boolean: ["blubb", "foo.bar", "foo.baz.biz", "foo.baz.buz"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        blubb: boolean;
        foo: {
          bar: boolean;
          baz: {
            biz: boolean;
            buz: boolean;
          };
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfDottedBooleanArgsWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: ["blubb", "foo.bar", "foo.baz.biz", "foo.baz.buz"],
    default: {
      blubb: "123",
      foo: {
        bar: 123,
        baz: {
          biz: new Date(),
        },
      },
      bla: new Date(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        blubb: boolean | string;
        foo: {
          bar: boolean | number;
          baz: {
            biz: boolean | Date;
            buz: boolean;
          };
        };
        bla: unknown;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfDottedStringArgs", function () {
  const argv = parseArgs([], {
    string: ["blubb", "foo.bar", "foo.baz.biz", "foo.baz.buz"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        blubb?: string | undefined;
        foo?: {
          bar?: string | undefined;
          baz?: {
            biz?: string | undefined;
            buz?: string | undefined;
          };
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfDottedStringArgsWithDefaults", function () {
  const argv = parseArgs([], {
    string: ["blubb", "foo.bar", "foo.baz.biz", "foo.baz.buz"],
    default: {
      blubb: true,
      foo: {
        bar: 123,
        baz: {
          biz: new Date(),
        },
      },
      bla: new Date(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        blubb: string | boolean;
        foo: {
          bar: string | number;
          baz: {
            biz: string | Date;
            buz?: string | undefined;
          };
        };
        bla: unknown;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfDottedStringAndBooleanArgs", function () {
  const argv = parseArgs([], {
    boolean: ["blubb", "foo.bar", "foo.baz.biz", "beep.bib.bub"],
    string: ["bla", "beep.boop", "beep.bib.bab", "foo.baz.buz"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        blubb: boolean;
        foo: {
          bar: boolean;
          baz: {
            biz: boolean;
            buz?: string | undefined;
          };
        };
        bla?: string | undefined;
        beep: {
          boop?: string | undefined;
          bib: {
            bab?: string | undefined;
            bub: boolean;
          };
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfDottedStringAndBooleanArgsWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: ["blubb", "foo.bar", "foo.baz.biz", "beep.bib.bub"],
    string: ["beep.boop", "beep.bib.bab", "foo.baz.buz"],
    default: {
      blubb: true,
      foo: {
        bar: 123,
        baz: {
          biz: new Date(),
        },
      },
      beep: {
        boop: true,
        bib: {
          bab: new Date(),
        },
      },
      bla: new Date(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        bla: unknown;
        blubb: boolean;
        foo: {
          bar: boolean | number;
          baz: {
            biz: boolean | Date;
            buz?: string | undefined;
          };
        };
        beep: {
          boop: string | boolean;
          bib: {
            bab: string | Date;
            bub: boolean;
          };
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfDottedStringAndBooleanArgsWithFlattedDefaults", function () {
  const argv = parseArgs([], {
    boolean: ["blubb", "foo.bar", "foo.baz.biz", "beep.bib.bub"],
    string: ["beep.boop", "beep.bib.bab", "foo.baz.buz"],
    default: {
      bla: new Date(),
      blubb: true,
      "foo.bar": 123,
      "foo.baz.biz": new Date(),
      "beep.boop": true,
      "beep.bib.bab": new Date(),
      "mee.moo": true,
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        bla: unknown;
        blubb: boolean;
        mee: unknown;
        foo: {
          bar: boolean | number;
          baz: {
            biz: boolean | Date;
            buz?: string | undefined;
          };
        };
        beep: {
          boop: string | boolean;
          bib: {
            bab: string | Date;
            bub: boolean;
          };
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfDottedArgsWithUnionDefaults", function () {
  const argv = parseArgs([], {
    string: ["foo.bar.baz"],
    boolean: ["beep.boop.bab"],
    default: {
      "foo": 1,
      "beep": new Date(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: number | {
          bar?: {
            baz?: string | undefined;
          } | undefined;
        };
        beep: Date | {
          boop: {
            bab: boolean;
          };
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfDottedArgsWithNestedUnionDefaults", function () {
  const argv = parseArgs([], {
    string: ["foo.bar.baz"],
    boolean: ["beep.boop.bab"],
    default: {
      "foo.bar": 1,
      "beep.boop": new Date(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: {
          bar: number | {
            baz?: string | undefined;
          };
        };
        beep: {
          boop: Date | {
            bab: boolean;
          };
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfArgsWithDottedDefaults", function () {
  const argv = parseArgs([], {
    string: ["foo"],
    default: {
      "foo.bar": 1,
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: string | {
          bar: number;
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

/** ------------------------ COLLECT OPTION -------------------------- */

Deno.test("typesOfCollectUnknownArgs", function () {
  const argv = parseArgs([], {
    collect: ["foo", "bar.baz"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: Array<unknown>;
        bar: {
          baz: Array<unknown>;
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfCollectArgs", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "dotted.beep"],
    string: ["bar", "dotted.boop"],
    collect: ["foo", "dotted.boop"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        bar?: string | undefined;
        dotted: {
          boop: Array<string>;
          beep: boolean;
        };
        foo: Array<boolean>;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfCollectArgsWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "dotted.beep"],
    string: ["bar", "dotted.boop"],
    collect: ["foo", "dotted.boop"],
    default: {
      bar: 123,
      dotted: {
        beep: new Date(),
        boop: /.*/,
      },
      foo: new TextDecoder(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        bar: number | string;
        foo: TextDecoder | Array<boolean>;
        dotted: {
          beep: boolean | Date;
          boop: RegExp | Array<string>;
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfCollectArgsWithSingleArgs", function () {
  const argv = parseArgs([], {
    boolean: ["foo"],
    collect: ["foo"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: Array<boolean>;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfCollectArgsWithEmptyTypeArray", function () {
  const argv = parseArgs([], {
    boolean: [],
    collect: ["foo"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: Array<unknown>;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfCollectArgsWithUnknownArgs", function () {
  const argv = parseArgs([], {
    boolean: ["bar"],
    collect: ["foo"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        bar: boolean;
        foo: Array<unknown>;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfCollectArgsWithKnownAndUnknownArgs", function () {
  const argv = parseArgs([], {
    boolean: ["foo"],
    collect: ["foo", "bar"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: Array<boolean>;
        bar: Array<unknown>;
        _: Array<string | number>;
      }
    >
  >(true);
});

/** -------------------------- NEGATABLE OPTIONS --------------------------- */

Deno.test("typesOfNegatableArgs", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "bar", "dotted.tick", "dotted.tock"],
    string: ["beep", "boop", "dotted.zig", "dotted.zag"],
    negatable: ["bar", "boop", "dotted.tick", "dotted.zig"],
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        beep?: string | undefined;
        boop?: string | false | undefined;
        dotted: {
          zig?: string | false | undefined;
          zag?: string | undefined;
          tick: boolean;
          tock: boolean;
        };
        foo: boolean;
        bar: boolean;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfCollectAllArgsWithDefaults", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "bar", "dotted.tick", "dotted.tock"],
    string: ["beep", "boop", "dotted.zig", "dotted.zag"],
    negatable: ["bar", "boop", "dotted.tick", "dotted.zig"],
    default: {
      bar: 123,
      boop: new TextDecoder(),
      dotted: {
        tick: new Date(),
        zig: /.*/,
      },
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo: boolean;
        beep?: string | undefined;
        bar: number | boolean;
        boop: string | false | TextDecoder;
        dotted: {
          zag?: string | undefined;
          tock: boolean;
          tick: boolean | Date;
          zig: string | false | RegExp;
        };
        _: Array<string | number>;
      }
    >
  >(true);
});

/** ----------------------------- ALIAS OPTION ----------------------------- */

Deno.test("typesOfAliasArgs", function () {
  const argv = parseArgs([], {
    boolean: ["foo"],
    string: ["beep"],
    alias: {
      foo: ["bar", "baz"],
      beep: "boop",
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        beep?: string | undefined;
        boop?: string | undefined;
        foo: boolean;
        bar: boolean;
        baz: boolean;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfAliasArgsWithOptions", function () {
  const argv = parseArgs([], {
    boolean: ["foo", "biz"],
    string: ["beep", "bib"],
    negatable: ["foo", "beep"],
    alias: {
      foo: "bar",
      beep: "boop",
      biz: "baz",
    },
    default: {
      foo: 1,
      beep: new Date(),
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        baz: boolean;
        biz: boolean;
        bib?: string | undefined;
        foo: number | boolean;
        bar: number | boolean;
        beep: string | false | Date;
        boop: string | false | Date;
        _: Array<string | number>;
      }
    >
  >(true);
});

/** ----------------------- OTHER TYPE TESTS ------------------------ */

Deno.test("typesOfDoubleDashOption", function () {
  const argv = parseArgs([], {
    boolean: true,
    string: ["foo"],
    "--": true,
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo?: string | undefined;
        _: Array<string | number>;
        "--": Array<string>;
      }
    >
  >(true);
});

Deno.test("typesOfNullishDefaults", function () {
  const argv = parseArgs([], {
    boolean: true,
    string: ["foo", "bar", "baz"],
    default: {
      bar: undefined,
      baz: null,
    },
  });
  assertType<
    IsExact<
      typeof argv,
      & { [x: string]: unknown }
      & {
        foo?: string | undefined;
        bar: string | undefined;
        baz: string | null;
        _: Array<string | number>;
      }
    >
  >(true);
});

Deno.test("typesOfParseGenerics", function () {
  const argv = parseArgs<{ foo?: number } & { bar: string }, true>([]);
  assertType<
    IsExact<
      typeof argv,
      {
        foo?: number | undefined;
        bar: string;
        _: Array<string | number>;
        "--": Array<string>;
      }
    >
  >(true);
});

Deno.test("typesOfArgsGenerics", function () {
  type ArgsResult = Args<{ foo?: number } & { bar: string }, true>;
  assertType<
    IsExact<
      ArgsResult,
      {
        foo?: number | undefined;
        bar: string;
        _: Array<string | number>;
        "--": Array<string>;
      }
    >
  >(true);
});

Deno.test("typesOfParseOptionsGenerics", function () {
  type Opts = ParseOptions<"foo", "bar" | "baz">;
  assertType<
    IsExact<
      Pick<Opts, "string">,
      { string?: "bar" | "baz" | ReadonlyArray<"bar" | "baz"> | undefined }
    >
  >(true);

  assertType<
    IsExact<
      Pick<Opts, "boolean">,
      { boolean?: "foo" | ReadonlyArray<"foo"> | undefined }
    >
  >(true);

  assertType<
    IsExact<
      Pick<Opts, "default">,
      {
        default?: {
          [x: string]: unknown;
          bar?: unknown;
          baz?: unknown;
          foo?: unknown;
        } | undefined;
      }
    >
  >(true);
});

Deno.test("typesOfParseOptionsGenericDefaults", function () {
  const opts: ParseOptions = {
    boolean: ["foo"],
    string: ["bar"],
  };

  const args = parseArgs([], opts);

  assertType<
    IsExact<
      typeof args,
      {
        // deno-lint-ignore no-explicit-any
        [x: string]: any;
        _: (string | number)[];
        "--"?: string[] | undefined;
      }
    >
  >(true);
});
