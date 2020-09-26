import { lstat, lstatSync } from "./_fs_lstat.ts";
import { assertThrows } from "../../testing/asserts.ts";
import { assertStats, assertStatsBigInt } from "./_fs_stat_test.ts";

Deno.test({
  name: "No callback Fn results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-ignore
        lstat(Deno.makeTempFileSync());
      },
      Error,
      "No callback function supplied"
    );
  },
});

Deno.test({
  name: "Test Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    lstat(file, (err, stat) => {
      if (err) throw err;
      assertStats(stat, Deno.lstatSync(file));
    });
  },
});

Deno.test({
  name: "Test Stats (sync)",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStats(lstatSync(file), Deno.lstatSync(file));
  },
});

Deno.test({
  name: "Test BigInt Stats",
  fn() {
    const file = Deno.makeTempFileSync();
    lstat(file, { bigint: true }, (err, stat) => {
      if (err) throw err;
      assertStatsBigInt(stat, Deno.lstatSync(file));
    });
  },
});

Deno.test({
  name: "Test BigInt Stats (sync)",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStatsBigInt(lstatSync(file, { bigint: true }), Deno.lstatSync(file));
  },
});
