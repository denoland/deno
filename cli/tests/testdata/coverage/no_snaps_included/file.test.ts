import { assertSnapshot } from "https://deno.land/std@0.136.0/testing/snapshot.ts";
import { truth } from "./file.ts";

Deno.test("the truth", () => {
  truth();
});

// Create snapshot in .snap file, but it shouldn't be in the coverage output
Deno.test("snapshot excluded from coverage", async (context) => {
  await assertSnapshot(context, {});
});
