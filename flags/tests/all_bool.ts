import { test, assertEqual } from "../../testing/mod.ts";
import { parse } from "../index.ts";

// flag boolean true (default all --args to boolean)
test(function flagBooleanTrue() {
  const argv = parse(["moo", "--honk", "cow"], {
    boolean: true
  });

  assertEqual(argv, {
    honk: true,
    _: ["moo", "cow"]
  });

  assertEqual(typeof argv.honk, "boolean");
});

// flag boolean true only affects double hyphen arguments without equals signs
test(function flagBooleanTrueOnlyAffectsDoubleDash() {
  var argv = parse(["moo", "--honk", "cow", "-p", "55", "--tacos=good"], {
    boolean: true
  });

  assertEqual(argv, {
    honk: true,
    tacos: "good",
    p: 55,
    _: ["moo", "cow"]
  });

  assertEqual(typeof argv.honk, "boolean");
});
