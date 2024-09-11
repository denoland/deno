// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { fstat, fstatSync } from "node:fs";
import { fail } from "@std/assert";
import { assertStats, assertStatsBigInt } from "./_fs_stat_test.ts";
import type { BigIntStats, Stats } from "node:fs";

Deno.test({
  name: "ASYNC: get a file Stats",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = await Deno.makeTempFile();
    using file = await Deno.open(filePath);

    await new Promise<Stats>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      fstat(file.rid, (err: Error | null, stat: Stats) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then(
        (stat) => {
          assertStats(stat, file.statSync());
        },
        () => fail(),
      )
      .finally(() => {
        Deno.removeSync(filePath);
      });
  },
});

Deno.test({
  name: "ASYNC: get a file BigInt Stats",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const filePath = await Deno.makeTempFile();
    using file = await Deno.open(filePath);

    await new Promise<BigIntStats>((resolve, reject) => {
      fstat(
        // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
        file.rid,
        { bigint: true },
        (err: Error | null, stat: BigIntStats) => {
          if (err) reject(err);
          resolve(stat);
        },
      );
    })
      .then(
        (stat) => assertStatsBigInt(stat, file.statSync()),
        () => fail(),
      )
      .finally(() => {
        Deno.removeSync(filePath);
      });
  },
});

Deno.test({
  name: "SYNC: get a file Stats",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    using file = Deno.openSync(filePath);

    try {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      assertStats(fstatSync(file.rid), file.statSync());
    } finally {
      Deno.removeSync(filePath);
    }
  },
});

Deno.test({
  name: "SYNC: get a file BigInt Stats",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const filePath = Deno.makeTempFileSync();
    using file = Deno.openSync(filePath);

    try {
      // HEAD
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      assertStatsBigInt(fstatSync(file.rid, { bigint: true }), file.statSync());
      //
      assertStatsBigInt(
        // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
        fstatSync(file.rid, { bigint: true }),
        file.statSync(),
      );
      //main
    } finally {
      Deno.removeSync(filePath);
    }
  },
});
