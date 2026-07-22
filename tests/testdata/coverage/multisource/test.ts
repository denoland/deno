import { foo } from "./foo.ts";
import { bar } from "./bar.ts";
import { qux } from "./baz/qux.ts";
import { quux } from "./baz/quux.ts";

Deno.test("foo", () => {
  foo(true);
  foo(false);
});

Deno.test("bar", () => {
  bar(false);
});

Deno.test("qux", () => {
  qux(true);
  qux(false);
});

Deno.test("quux", () => {
  quux(false);
});

// Function constructor or eval function generates a new script source internally.
// This call ensures that the coverage data for the eval script source is not generated.
eval("console.log(1)");
