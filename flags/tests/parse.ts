// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function _arseArgs() {
  assertEq(parse(["--no-moo"]), { moo: false, _: [] });
  assertEq(parse(["-v", "a", "-v", "b", "-v", "c"]), {
    v: ["a", "b", "c"],
    _: []
  });
});

test(function comprehensive() {
  assertEq(
    parse([
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
      "--",
      "--not-a-flag",
      "eek"
    ]),
    {
      c: true,
      a: true,
      t: true,
      s: "woo",
      h: "awesome",
      b: true,
      bool: true,
      key: "value",
      multi: ["quux", "baz"],
      meep: false,
      name: "meowmers",
      _: ["bare", "--not-a-flag", "eek"]
    }
  );
});

test(function flagBoolean() {
  const argv = parse(["-t", "moo"], { boolean: "t" });
  assertEq(argv, { t: true, _: ["moo"] });
  assertEq(typeof argv.t, "boolean");
});

test(function flagBooleanValue() {
  const argv = parse(["--verbose", "false", "moo", "-t", "true"], {
    boolean: ["t", "verbose"],
    default: { verbose: true }
  });

  assertEq(argv, {
    verbose: false,
    t: true,
    _: ["moo"]
  });

  assertEq(typeof argv.verbose, "boolean");
  assertEq(typeof argv.t, "boolean");
});

test(function newlinesInParams() {
  const args = parse(["-s", "X\nX"]);
  assertEq(args, { _: [], s: "X\nX" });

  // reproduce in bash:
  // VALUE="new
  // line"
  // deno program.js --s="$VALUE"
  const args2 = parse(["--s=X\nX"]);
  assertEq(args2, { _: [], s: "X\nX" });
});

test(function strings() {
  const s = parse(["-s", "0001234"], { string: "s" }).s;
  assertEq(s, "0001234");
  assertEq(typeof s, "string");

  const x = parse(["-x", "56"], { string: "x" }).x;
  assertEq(x, "56");
  assertEq(typeof x, "string");
});

test(function stringArgs() {
  const s = parse(["  ", "  "], { string: "_" })._;
  assertEq(s.length, 2);
  assertEq(typeof s[0], "string");
  assertEq(s[0], "  ");
  assertEq(typeof s[1], "string");
  assertEq(s[1], "  ");
});

test(function emptyStrings() {
  const s = parse(["-s"], { string: "s" }).s;
  assertEq(s, "");
  assertEq(typeof s, "string");

  const str = parse(["--str"], { string: "str" }).str;
  assertEq(str, "");
  assertEq(typeof str, "string");

  const letters = parse(["-art"], {
    string: ["a", "t"]
  });

  assertEq(letters.a, "");
  assertEq(letters.r, true);
  assertEq(letters.t, "");
});

test(function stringAndAlias() {
  const x = parse(["--str", "000123"], {
    string: "s",
    alias: { s: "str" }
  });

  assertEq(x.str, "000123");
  assertEq(typeof x.str, "string");
  assertEq(x.s, "000123");
  assertEq(typeof x.s, "string");

  const y = parse(["-s", "000123"], {
    string: "str",
    alias: { str: "s" }
  });

  assertEq(y.str, "000123");
  assertEq(typeof y.str, "string");
  assertEq(y.s, "000123");
  assertEq(typeof y.s, "string");
});

test(function slashBreak() {
  assertEq(parse(["-I/foo/bar/baz"]), { I: "/foo/bar/baz", _: [] });
  assertEq(parse(["-xyz/foo/bar/baz"]), {
    x: true,
    y: true,
    z: "/foo/bar/baz",
    _: []
  });
});

test(function alias() {
  const argv = parse(["-f", "11", "--zoom", "55"], {
    alias: { z: "zoom" }
  });
  assertEq(argv.zoom, 55);
  assertEq(argv.z, argv.zoom);
  assertEq(argv.f, 11);
});

test(function multiAlias() {
  const argv = parse(["-f", "11", "--zoom", "55"], {
    alias: { z: ["zm", "zoom"] }
  });
  assertEq(argv.zoom, 55);
  assertEq(argv.z, argv.zoom);
  assertEq(argv.z, argv.zm);
  assertEq(argv.f, 11);
});

test(function nestedDottedObjects() {
  const argv = parse([
    "--foo.bar",
    "3",
    "--foo.baz",
    "4",
    "--foo.quux.quibble",
    "5",
    "--foo.quux.o_O",
    "--beep.boop"
  ]);

  assertEq(argv.foo, {
    bar: 3,
    baz: 4,
    quux: {
      quibble: 5,
      o_O: true
    }
  });
  assertEq(argv.beep, { boop: true });
});
