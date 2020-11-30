import { relativePath, resolvePath } from "./mod.ts";
import { assertEquals } from "../testing/asserts.ts";

const pwd = Deno.cwd();

Deno.test("resolveZeroLength", function () {
  // resolvePath, internally ignores all the zero-length strings and returns the
  // current working directory
  assertEquals(resolvePath(""), pwd);
  assertEquals(resolvePath("", ""), pwd);
});

Deno.test("relativeZeroLength", function () {
  // relative, internally calls resolve. So, '' is actually the current
  // directory
  assertEquals(relativePath("", pwd), "");
  assertEquals(relativePath(pwd, ""), "");
  assertEquals(relativePath(pwd, pwd), "");
});
