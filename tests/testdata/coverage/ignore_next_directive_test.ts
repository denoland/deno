import { used } from "./ignore_next_directive.ts";

Deno.test("used", function () {
  used(false);
});
