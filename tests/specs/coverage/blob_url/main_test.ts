import { foo } from "./main.ts";

Deno.test("test", async () => {
  await foo();
});
