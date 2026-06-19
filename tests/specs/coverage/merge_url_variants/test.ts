import { a } from "./source.ts";
import { b } from "./source.ts#fragment";
import { c } from "./source.ts?search";

Deno.test("a", () => {
  a();
});

Deno.test("b", () => {
  b();
});

Deno.test("c", () => {
  c();
});
