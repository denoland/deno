import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import { parse } from "../index.ts";

test(function hyphen() {
  assertEqual(parse(["-n", "-"]), { n: "-", _: [] });
  assertEqual(parse(["-"]), { _: ["-"] });
  assertEqual(parse(["-f-"]), { f: "-", _: [] });
  assertEqual(parse(["-b", "-"], { boolean: "b" }), { b: true, _: ["-"] });
  assertEqual(parse(["-s", "-"], { string: "s" }), { s: "-", _: [] });
});

test(function doubleDash() {
  assertEqual(parse(["-a", "--", "b"]), { a: true, _: ["b"] });
  assertEqual(parse(["--a", "--", "b"]), { a: true, _: ["b"] });
  assertEqual(parse(["--a", "--", "b"]), { a: true, _: ["b"] });
});

test(function moveArgsAfterDoubleDashIntoOwnArray() {
  assertEqual(
    parse(["--name", "John", "before", "--", "after"], { "--": true }),
    { name: "John", _: ["before"], "--": ["after"] }
  );
});
