// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { watch } from "node:fs";
<<<<<<< HEAD
import { assertEquals } from "../../../../test_util/std/assert/mod.ts";
=======
import { assertEquals } from "../../../../test_util/std/testing/asserts.ts";
>>>>>>> 172e5f0a0 (1.38.5 (#21469))

function wait(time: number) {
  return new Promise((resolve) => {
    setTimeout(resolve, time);
  });
}

Deno.test({
  name: "watching a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    const result: Array<[string, string | null]> = [];
    const watcher = watch(
      file,
      (eventType, filename) => result.push([eventType, filename]),
    );
    await wait(100);
    Deno.writeTextFileSync(file, "something");
    await wait(100);
    watcher.close();
    await wait(100);
    assertEquals(result.length >= 1, true);
  },
});
