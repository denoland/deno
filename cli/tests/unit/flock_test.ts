// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, unitTest } from "./test_util.ts";

unitTest(
  { perms: { read: true, run: true, hrtime: true } },
  async function flockFileSync() {
    const path = "cli/tests/testdata/fixture.json";
    const script = (exclusive: boolean, wait: number) => `
          const { rid } = Deno.openSync("${path}");
          Deno.flockSync(rid, ${exclusive ? "true" : "false"});
          await new Promise(res => setTimeout(res, ${wait}));
          Deno.funlockSync(rid);
      `;
    const run = (e: boolean, w: number) =>
      Deno.run({ cmd: [Deno.execPath(), "eval", "--unstable", script(e, w)] });
    const firstBlocksSecond = async (
      first: boolean,
      second: boolean,
    ): Promise<boolean> => {
      const firstPs = run(first, 1000);
      await new Promise((res) => setTimeout(res, 250));
      const start = performance.now();
      const secondPs = run(second, 0);
      await secondPs.status();
      const didBlock = (performance.now() - start) > 500;
      firstPs.close();
      secondPs.close();
      return didBlock;
    };

    assertEquals(
      await firstBlocksSecond(true, false),
      true,
      "exclusive blocks shared",
    );
    assertEquals(
      await firstBlocksSecond(false, true),
      true,
      "shared blocks exclusive",
    );
    assertEquals(
      await firstBlocksSecond(true, true),
      true,
      "exclusive blocks exclusive",
    );
    assertEquals(
      await firstBlocksSecond(false, false),
      false,
      "shared does not block shared",
    );
  },
);

unitTest(
  { perms: { read: true, run: true, hrtime: true } },
  async function flockFileAsync() {
    const path = "cli/tests/testdata/fixture.json";
    const script = (exclusive: boolean, wait: number) => `
          const { rid } = await Deno.open("${path}");
          await Deno.flock(rid, ${exclusive ? "true" : "false"});
          await new Promise(res => setTimeout(res, ${wait}));
          await Deno.funlock(rid);
      `;
    const run = (e: boolean, w: number) =>
      Deno.run({ cmd: [Deno.execPath(), "eval", "--unstable", script(e, w)] });
    const firstBlocksSecond = async (
      first: boolean,
      second: boolean,
    ): Promise<boolean> => {
      const firstPs = run(first, 1000);
      await new Promise((res) => setTimeout(res, 250));
      const start = performance.now();
      const secondPs = run(second, 0);
      await secondPs.status();
      const didBlock = (performance.now() - start) > 500;
      firstPs.close();
      secondPs.close();
      return didBlock;
    };

    assertEquals(
      await firstBlocksSecond(true, false),
      true,
      "exclusive blocks shared",
    );
    assertEquals(
      await firstBlocksSecond(false, true),
      true,
      "shared blocks exclusive",
    );
    assertEquals(
      await firstBlocksSecond(true, true),
      true,
      "exclusive blocks exclusive",
    );
    assertEquals(
      await firstBlocksSecond(false, false),
      false,
      "shared does not block shared",
    );
  },
);
