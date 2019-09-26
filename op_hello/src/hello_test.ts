// TODO  We need some way to import test modules.
// Attempt one:
//
//   import { test } from "../../js/test_util.ts";
//
// Here it is referencing files across crate boundaries, which will break
// 'cargo package' and means the crate is not useable outside the deno tree.
// This might be okay for a first pass, but it's not the best solution.
//
// Attempt two:
// we invent a new URL for referencing files in other crates.
// this is magic and not browser compatible.. Browser compatibility for
// ops is not so important.
import { test } from "crate://deno_std@0.19.0/testing/mod.ts";
import { hello } from "./hello.ts";

test("hello test", () => {
  hello();
});
