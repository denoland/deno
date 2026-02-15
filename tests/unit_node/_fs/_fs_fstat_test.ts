// Copyright 2018-2026 the Deno authors. MIT license.

import { closeSync, fstat, fstatSync, openSync } from "node:fs";
import { fail } from "@std/assert";
import { assertStats, assertStatsBigInt } from "../_test_utils.ts";
import type { BigIntStats, Stats } from "node:fs";

Deno.test({
  name: "ASYNC: get a file Stats",
  async fn() {
    const filePath = await Deno.makeTempFile();
    const fd = openSync(filePath, "r");

    await new Promise<Stats>((resolve, reject) => {
      fstat(fd, (err: Error | null, stat: Stats) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then(
        (stat) => {
          using file = Deno.openSync(filePath);
          assertStats(stat, file.statSync());
        },
        () => fail(),
      )
      .finally(() => {
        closeSync(fd);
        Deno.removeSync(filePath);
      });
  },
});

Deno.test({
  name: "ASYNC: get a file BigInt Stats",
  async fn() {
    const filePath = await Deno.makeTempFile();
    const fd = openSync(filePath, "r");

    await new Promise<BigIntStats>((resolve, reject) => {
      fstat(
        fd,
        { bigint: true },
        (err: Error | null, stat: BigIntStats) => {
          if (err) reject(err);
          resolve(stat);
        },
      );
    })
      .then(
        (stat) => {
          using file = Deno.openSync(filePath);
          assertStatsBigInt(stat, file.statSync());
        },
        () => fail(),
      )
      .finally(() => {
        closeSync(fd);
        Deno.removeSync(filePath);
      });
  },
});

Deno.test({
  name: "SYNC: get a file Stats",
  fn() {
    const filePath = Deno.makeTempFileSync();
    const fd = openSync(filePath, "r");

    try {
      using file = Deno.openSync(filePath);
      assertStats(fstatSync(fd), file.statSync());
    } finally {
      closeSync(fd);
      Deno.removeSync(filePath);
    }
  },
});

Deno.test({
  name: "SYNC: get a file BigInt Stats",
  fn() {
    const filePath = Deno.makeTempFileSync();
    const fd = openSync(filePath, "r");

    try {
      using file = Deno.openSync(filePath);
      assertStatsBigInt(
        fstatSync(fd, { bigint: true }),
        file.statSync(),
      );
    } finally {
      closeSync(fd);
      Deno.removeSync(filePath);
    }
  },
});
