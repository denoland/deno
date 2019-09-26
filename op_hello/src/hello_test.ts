// TODO  We need some way to import test modules.
// Attempt one:
//
//   import { test, assert } from "../../js/test_util.ts";
//
// Here it is referencing files across crate boundaries, which will break
// 'cargo package' and means the crate is not useable outside the deno tree.
// This might be okay for a first pass, but it's not the best solution.
//
// Attempt two:
// we invent a new URL for referencing files in other crates.
// this is magic and not browser compatible.. Browser compatibility for
// ops is not so important.
import { test, assert } from "crate://deno_cli_snapshots@0.19.0/test_util.ts";

import { hello } from "./hello.ts";

test(function testHello(): void {
  hello();
});
