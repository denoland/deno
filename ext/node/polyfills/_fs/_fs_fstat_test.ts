// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { fstat, fstatSync } from "./_fs_fstat.ts";
import { fail } from "../../testing/asserts.ts";
import { assertStats, assertStatsBigInt } from "./_fs_stat_test.ts";
import type { BigIntStats, Stats } from "./_fs_stat.ts";

Deno.test({
  name: "ASYNC: get a file Stats",
  async fn() {
    const file = await Deno.makeTempFile();
    const { rid } = await Deno.open(file);

    await new Promise<Stats>((resolve, reject) => {
      fstat(rid, (err: Error | null, stat: Stats) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then(
        (stat) => {
          assertStats(stat, Deno.fstatSync(rid));
        },
        () => fail(),
      )
      .finally(() => {
        Deno.removeSync(file);
        Deno.close(rid);
      });
  },
});

Deno.test({
  name: "ASYNC: get a file BigInt Stats",
  async fn() {
    const file = await Deno.makeTempFile();
    const { rid } = await Deno.open(file);

    await new Promise<BigIntStats>((resolve, reject) => {
      fstat(rid, { bigint: true }, (err: Error | null, stat: BigIntStats) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then(
        (stat) => assertStatsBigInt(stat, Deno.fstatSync(rid)),
        () => fail(),
      )
      .finally(() => {
        Deno.removeSync(file);
        Deno.close(rid);
      });
  },
});

Deno.test({
  name: "SYNC: get a file Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    const { rid } = Deno.openSync(file);

    try {
      assertStats(fstatSync(rid), Deno.fstatSync(rid));
    } finally {
      Deno.removeSync(file);
      Deno.close(rid);
    }
  },
});

Deno.test({
  name: "SYNC: get a file BigInt Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    const { rid } = Deno.openSync(file);

    try {
      assertStatsBigInt(fstatSync(rid, { bigint: true }), Deno.fstatSync(rid));
    } finally {
      Deno.removeSync(file);
      Deno.close(rid);
    }
  },
});
