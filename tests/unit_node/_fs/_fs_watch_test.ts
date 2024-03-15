// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { Stats, unwatchFile, watch, watchFile } from "node:fs";
import { assert, assertEquals } from "@std/assert/mod.ts";

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

Deno.test({
  name: "watching a file with options",
  async fn() {
    const file = Deno.makeTempFileSync();
    const watcher = watchFile(
      file,
      () => {},
    );
    await wait(100);
    unwatchFile(file);
  },
});
