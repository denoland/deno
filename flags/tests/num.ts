import { test, assertEqual } from "../../testing/mod.ts";
import { parse } from "../index.ts";

test(function nums() {
  const argv = parse([
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
    "789"
  ]);
  assertEqual(argv, {
    x: 1234,
    y: 5.67,
    z: 1e7,
    w: "10f",
    hex: 0xdeadbeef,
    _: [789]
  });
  assertEqual(typeof argv.x, "number");
  assertEqual(typeof argv.y, "number");
  assertEqual(typeof argv.z, "number");
  assertEqual(typeof argv.w, "string");
  assertEqual(typeof argv.hex, "number");
  assertEqual(typeof argv._[0], "number");
});

test(function alreadyNumber() {
  const argv = parse(["-x", 1234, 789]);
  assertEqual(argv, { x: 1234, _: [789] });
  assertEqual(typeof argv.x, "number");
  assertEqual(typeof argv._[0], "number");
});
