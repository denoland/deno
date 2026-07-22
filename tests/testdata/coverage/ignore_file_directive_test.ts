import { used } from "./ignore_file_directive.ts";

Deno.test("used", function () {
  used();
});
