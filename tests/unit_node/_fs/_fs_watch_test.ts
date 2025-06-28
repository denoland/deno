// Copyright 2018-2025 the Deno authors. MIT license.
import { unwatchFile, watch, watchFile } from "node:fs";
import { watch as watchPromise } from "node:fs/promises";
import { assert, assertEquals } from "@std/assert";
import { spy } from "@std/testing/mock";

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
  // TODO(bartlomieju): this test is flaky on CI
  ignore: true,
  async fn() {
    const file = Deno.makeTempFileSync();
    const spyFn = spy();
    watchFile(
      file,
      { interval: 10 },
      spyFn,
    );
    await wait(100);
    assertEquals(spyFn.calls.length, 0);
    await Deno.writeTextFile(file, "something");
    await wait(100);
    assertEquals(spyFn.calls.length, 1);
    unwatchFile(file);
    await wait(100);
    assertEquals(spyFn.calls.length, 1);
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

Deno.test({
  name: "node [fs/promises] watch should return async iterable",
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "foo");

    const result: { eventType: string; filename: string | null }[] = [];

    const controller = new AbortController();
    const watcher = watchPromise(file, {
      // Node types resolved by the LSP clash with ours
      // deno-lint-ignore no-explicit-any
      signal: controller.signal as any,
    });

    const deferred = Promise.withResolvers<void>();
    let stopLength = 0;
    setTimeout(async () => {
      Deno.writeTextFileSync(file, "something");
      controller.abort();
      stopLength = result.length;
      await wait(100);
      Deno.writeTextFileSync(file, "something else");
      await wait(100);
      deferred.resolve();
    }, 100);

    for await (const event of watcher) {
      result.push(event);
    }
    await deferred.promise;

    assertEquals(result.length, stopLength);
    assert(
      result.every((item) =>
        typeof item.eventType === "string" && typeof item.filename === "string"
      ),
    );
  },
});
