import { nextTick } from "../../../../test_util/std/node/_next_tick.ts";

// https://github.com/denoland/deno_std/issues/1651

Deno.test("test 1", async () => {
  await new Promise<void>((resolve) => nextTick(resolve));
});

Deno.test("test 2", async () => {
  await new Promise<void>((resolve) => nextTick(resolve));
});
