import { lstat, lstatSync } from "./_fs_lstat.ts";
import { assert, assertStringIncludes, fail } from "../../testing/asserts.ts";
import { assertStats, assertStatsBigInt } from "./_fs_stat_test.ts";
import type { BigIntStats, Stats } from "./_fs_stat.ts";

Deno.test({
  name: "ASYNC: get a file Stats (lstat)",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<Stats>((resolve, reject) => {
      lstat(file, (err, stat) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then((stat) => {
        assertStats(stat, Deno.lstatSync(file));
      }, () => fail())
      .finally(() => {
        Deno.removeSync(file);
      });
  },
});

Deno.test({
  name: "SYNC: get a file Stats (lstat)",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStats(lstatSync(file), Deno.lstatSync(file));
  },
});

Deno.test({
  name: "ASYNC: get a file BigInt Stats (lstat)",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<BigIntStats>((resolve, reject) => {
      lstat(file, { bigint: true }, (err, stat) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then(
        (stat) => assertStatsBigInt(stat, Deno.lstatSync(file)),
        () => fail(),
      )
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: BigInt Stats (lstat)",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStatsBigInt(lstatSync(file, { bigint: true }), Deno.lstatSync(file));
  },
});

Deno.test("[std/node/fs] lstat callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempFile = await Deno.makeTempFile();
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { lstat } from "${
        new URL("./_fs_lstat.ts", import.meta.url).href
      }";

      lstat(${JSON.stringify(tempFile)}, (err) => {
        // If the bug is present and the callback is called again with an error,
        // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
        if (!err) throw new Error("success");
      });`,
    ],
    stderr: "piped",
  });
  const status = await p.status();
  const stderr = new TextDecoder().decode(await Deno.readAll(p.stderr));
  p.close();
  p.stderr.close();
  await Deno.remove(tempFile);
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
