import { used } from "./ignore_range_directive.ts";

Deno.test("used", function () {
  used(false);
});
