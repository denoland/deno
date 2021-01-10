import { branch } from "./subdir/branch.ts";

Deno.test("branch", function () {
  branch(true);
});
