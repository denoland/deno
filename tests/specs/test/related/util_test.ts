import { id } from "./util.ts";

Deno.test("id", () => {
  if (id(5) !== 5) throw new Error("fail");
});
