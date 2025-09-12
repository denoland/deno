// Copyright 2018-2025 the Deno authors. MIT license.
import {
  assertCallbackErrorUncaught,
  assertStats,
  assertStatsBigInt,
} from "../_test_utils.ts";
import { BigIntStats, stat, Stats, statSync } from "node:fs";
import { assert, assertEquals, fail } from "@std/assert";

Deno.test({
  name: "ASYNC: get a file Stats",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<Stats>((resolve, reject) => {
      stat(file, (err, stat) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then((stat) => assertStats(stat, Deno.statSync(file)), () => fail())
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: get a file Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStats(statSync(file), Deno.statSync(file));
  },
});

Deno.test({
  name: "ASYNC: get a file BigInt Stats",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<BigIntStats>((resolve, reject) => {
      stat(file, { bigint: true }, (err, stat) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then(
        (stat) => assertStatsBigInt(stat, Deno.statSync(file)),
        () => fail(),
      )
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: get a file BigInt Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStatsBigInt(statSync(file, { bigint: true }), Deno.statSync(file));
  },
});

Deno.test("[std/node/fs] stat callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { stat } from ${JSON.stringify(importUrl)}`,
    invocation: `stat(${JSON.stringify(tempFile)}, `,
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});

Deno.test({
  name: "[std/node/fs] stat default methods",
  fn() {
    // stats ctor is private
    // deno-lint-ignore no-explicit-any
    const stats = new (Stats as any)();
    assertEquals(stats.isFile(), false);
    assertEquals(stats.isDirectory(), false);
    assertEquals(stats.isBlockDevice(), false);
    assertEquals(stats.isCharacterDevice(), false);
    assertEquals(stats.isSymbolicLink(), false);
    assertEquals(stats.isFIFO(), false);
    assertEquals(stats.isSocket(), false);
  },
});

Deno.test({
  name: "[node/fs] stat invalid path error",
  async fn() {
    try {
      await new Promise<Stats>((resolve, reject) => {
        stat(
          // deno-lint-ignore no-explicit-any
          undefined as any,
          (err, stats) => err ? reject(err) : resolve(stats),
        );
      });
      fail();
    } catch (err) {
      assert(err instanceof TypeError);
      // deno-lint-ignore no-explicit-any
      assertEquals((err as any).code, "ERR_INVALID_ARG_TYPE");
    }
  },
});

Deno.test({
  name: "[node/fs] statSync invalid path error",
  fn() {
    try {
      // deno-lint-ignore no-explicit-any
      statSync(undefined as any);
      fail();
    } catch (err) {
      assert(err instanceof TypeError);
      // deno-lint-ignore no-explicit-any
      assertEquals((err as any).code, "ERR_INVALID_ARG_TYPE");
    }
  },
});
