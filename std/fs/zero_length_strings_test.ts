import { resolvePath } from "./mod.ts";
import { assertEquals } from "../testing/asserts.ts";

const pwd = Deno.cwd();

Deno.test("resolveZeroLength", function () {
  // resolvePath, internally ignores all the zero-length strings and returns the
  // current working directory
  assertEquals(resolvePath(""), pwd);
  assertEquals(resolvePath("", ""), pwd);
});
