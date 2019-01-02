import { test, assertEqual } from "../../testing/mod.ts";
import { parse } from "../index.ts";

test(function booleanDefaultTrue() {
  const argv = parse([], {
    boolean: "sometrue",
    default: { sometrue: true }
  });
  assertEqual(argv.sometrue, true);
});

test(function booleanDefaultFalse() {
  const argv = parse([], {
    boolean: "somefalse",
    default: { somefalse: false }
  });
  assertEqual(argv.somefalse, false);
});

test(function booleanDefaultNull() {
  const argv = parse([], {
    boolean: "maybe",
    default: { maybe: null }
  });
  assertEqual(argv.maybe, null);
  const argv2 = parse(["--maybe"], {
    boolean: "maybe",
    default: { maybe: null }
  });
  assertEqual(argv2.maybe, true);
});
