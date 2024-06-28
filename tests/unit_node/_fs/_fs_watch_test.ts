// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { unwatchFile, watch, watchFile } from "node:fs";
import { assertEquals } from "@std/assert/mod.ts";

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
    watchFile(
      file,
      () => {},
    );
    await wait(100);
    unwatchFile(file);
  },
});

Deno.test({
  name: "watch.unref() should work",
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const file = Deno.makeTempFileSync();
    const watcher = watch(file, () => {});
    // Wait for the watcher to be initialized
    await wait(10);
    // @ts-ignore node types are outdated in deno.
    watcher.unref();
  },
});
