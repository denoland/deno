// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEquals } from "./test_util.ts";
import { runIfMain } from "../../std/testing/mod.ts";

async function captureEvents(
  watcher: Deno.FsWatcher,
  expectedLength: number
): Promise<void> {
  const events = [];
  for await (const event of watcher) {
    events.push(event);
    console.error("got event!", event);
  }
  assertEquals(events.length, expectedLength);
}

testPerm({ read: true, write: true }, async function fsWatcher(): Promise<
  void
> {
  const testDir = await Deno.makeTempDir();
  const file1 = testDir + "/file1.txt";
  const file2 = testDir + "/file2.txt";

  const watcher = Deno.watch(testDir, { recursive: true });
  // start capturing events in the background
  await Deno.writeFile(file1, new Uint8Array([0, 1, 2]));
  await Deno.writeFile(file2, new Uint8Array([0, 1, 2]));
  await Deno.remove(file1);

  setTimeout(() => {
    watcher.close();
  }, 750);
  await captureEvents(watcher, 6);
});

runIfMain(import.meta);
