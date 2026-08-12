import { greet } from "./main.ts";

Deno.test("greet", () => {
  greet("world");
});
