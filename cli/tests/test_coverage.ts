import { returnsFoo2 } from "./subdir/mod1.ts";

Deno.test("returnsFooSuccess", function () {
  returnsFoo2();
});
