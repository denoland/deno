import { complex } from "./subdir/complex.ts";

Deno.test("complex", function() {
  complex("foo", "bar", "baz");
});
