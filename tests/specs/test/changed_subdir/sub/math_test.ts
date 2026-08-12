import { add } from "./math.ts";

Deno.test("add", () => {
  if (add(1, 2) !== 3) throw new Error("fail");
});
