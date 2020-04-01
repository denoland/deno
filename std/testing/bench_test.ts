const { test } = Deno;
import { bench, runBenchmarks } from "./bench.ts";

import "./bench_example.ts";

test({
  name: "benching",

  fn: async function (): Promise<void> {
    bench(function forIncrementX1e9(b): void {
      b.start();
      for (let i = 0; i < 1e9; i++);
      b.stop();
    });

    bench(function forDecrementX1e9(b): void {
      b.start();
      for (let i = 1e9; i > 0; i--);
      b.stop();
    });

    bench(async function forAwaitFetchDenolandX10(b): Promise<void> {
      b.start();
      for (let i = 0; i < 10; i++) {
        const r = await fetch("https://deno.land/");
        await r.text();
      }
      b.stop();
    });

    bench(async function promiseAllFetchDenolandX10(b): Promise<void> {
      const urls = new Array(10).fill("https://deno.land/");
      b.start();
      await Promise.all(
        urls.map(
          async (denoland: string): Promise<void> => {
            const r = await fetch(denoland);
            await r.text();
          }
        )
      );
      b.stop();
    });

    bench({
      name: "runs100ForIncrementX1e6",
      runs: 100,
      func(b): void {
        b.start();
        for (let i = 0; i < 1e6; i++);
        b.stop();
      },
    });

    bench(function throwing(b): void {
      b.start();
      // Throws bc the timer's stop method is never called
    });

    await runBenchmarks({ skip: /throw/ });
  },
});
