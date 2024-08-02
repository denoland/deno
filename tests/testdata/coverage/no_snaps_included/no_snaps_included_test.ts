import { assertSnapshot } from "@std/testing/snapshot";
import { truth } from "./no_snaps_included.ts";

Deno.test("the truth", () => {
  truth();
});

// Create snapshot in .snap file, but it shouldn't be in the coverage output
Deno.test("snapshot excluded from coverage", async (context) => {
  await assertSnapshot(context, {});
});
