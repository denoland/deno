import { returnsHi } from "./subdir/mod1.ts";

Deno.test("returnsHiSuccess", function () {
  returnsHi();
});
