import { tryCatchFinally } from "./try_catch_finally_returns.ts";

Deno.test("tryCatchFinally", function () {
  tryCatchFinally(false);
  tryCatchFinally(true);
});
