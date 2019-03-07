// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEquals } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function flagBooleanDefaultFalse() {
  const argv = parse(["moo"], {
    boolean: ["t", "verbose"],
    default: { verbose: false, t: false }
  });

  assertEquals(argv, {
    verbose: false,
    t: false,
    _: ["moo"]
  });

  assertEquals(typeof argv.verbose, "boolean");
  assertEquals(typeof argv.t, "boolean");
});

test(function booleanGroups() {
  const argv = parse(["-x", "-z", "one", "two", "three"], {
    boolean: ["x", "y", "z"]
  });

  assertEquals(argv, {
    x: true,
    y: false,
    z: true,
    _: ["one", "two", "three"]
  });

  assertEquals(typeof argv.x, "boolean");
  assertEquals(typeof argv.y, "boolean");
  assertEquals(typeof argv.z, "boolean");
});

test(function booleanAndAliasWithChainableApi() {
  const aliased = ["-h", "derp"];
  const regular = ["--herp", "derp"];
  const opts = {
    herp: { alias: "h", boolean: true }
  };
  const aliasedArgv = parse(aliased, {
    boolean: "herp",
    alias: { h: "herp" }
  });
  const propertyArgv = parse(regular, {
    boolean: "herp",
    alias: { h: "herp" }
  });
  const expected = {
    herp: true,
    h: true,
    _: ["derp"]
  };

  assertEquals(aliasedArgv, expected);
  assertEquals(propertyArgv, expected);
});

test(function booleanAndAliasWithOptionsHash() {
  const aliased = ["-h", "derp"];
  const regular = ["--herp", "derp"];
  const opts = {
    alias: { h: "herp" },
    boolean: "herp"
  };
  const aliasedArgv = parse(aliased, opts);
  const propertyArgv = parse(regular, opts);
  const expected = {
    herp: true,
    h: true,
    _: ["derp"]
  };
  assertEquals(aliasedArgv, expected);
  assertEquals(propertyArgv, expected);
});

test(function booleanAndAliasArrayWithOptionsHash() {
  const aliased = ["-h", "derp"];
  const regular = ["--herp", "derp"];
  const alt = ["--harp", "derp"];
  const opts = {
    alias: { h: ["herp", "harp"] },
    boolean: "h"
  };
  const aliasedArgv = parse(aliased, opts);
  const propertyArgv = parse(regular, opts);
  const altPropertyArgv = parse(alt, opts);
  const expected = {
    harp: true,
    herp: true,
    h: true,
    _: ["derp"]
  };
  assertEquals(aliasedArgv, expected);
  assertEquals(propertyArgv, expected);
  assertEquals(altPropertyArgv, expected);
});

test(function booleanAndAliasUsingExplicitTrue() {
  const aliased = ["-h", "true"];
  const regular = ["--herp", "true"];
  const opts = {
    alias: { h: "herp" },
    boolean: "h"
  };
  const aliasedArgv = parse(aliased, opts);
  const propertyArgv = parse(regular, opts);
  const expected = {
    herp: true,
    h: true,
    _: []
  };

  assertEquals(aliasedArgv, expected);
  assertEquals(propertyArgv, expected);
});

// regression, see https://github.com/substack/node-optimist/issues/71
// boolean and --x=true
test(function booleanAndNonBoolean() {
  const parsed = parse(["--boool", "--other=true"], {
    boolean: "boool"
  });

  assertEquals(parsed.boool, true);
  assertEquals(parsed.other, "true");

  const parsed2 = parse(["--boool", "--other=false"], {
    boolean: "boool"
  });

  assertEquals(parsed2.boool, true);
  assertEquals(parsed2.other, "false");
});

test(function booleanParsingTrue() {
  const parsed = parse(["--boool=true"], {
    default: {
      boool: false
    },
    boolean: ["boool"]
  });

  assertEquals(parsed.boool, true);
});

test(function booleanParsingFalse() {
  const parsed = parse(["--boool=false"], {
    default: {
      boool: true
    },
    boolean: ["boool"]
  });

  assertEquals(parsed.boool, false);
});
