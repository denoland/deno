// TODO here we're referencing files accross crate boundaries, which has two
// problems:
// 1. "cargo package" breaks when you do this.
// 2. Using this crate outside of the deno tree becomes impossible.
import { test, assert } from "./src/js/test_util.ts";
import { hello } from "./hello.ts";

test(function testHello(): void {
  hello();
});
