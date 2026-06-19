import { a } from "./repro.ts";
import { b } from "./repro.ts#fragment";
import { c } from "./repro.ts?search";

Deno.test("a", () => {
  a();
});

Deno.test("b", () => {
  b();
});

Deno.test("c", () => {
  c();
});
