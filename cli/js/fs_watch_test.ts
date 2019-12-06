// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEquals } from "./test_util.ts";

async function captureEvents(
  watcher: Deno.FsWatcher,
  expectedLength: number
): Promise<void> {
  const events = [];
  for await (const event of watcher) {
    events.push(event);
    console.error("got event!", event);
  }
  console.error("captured events", events);
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
  console.error("written file1");
  await Deno.writeFile(file2, new Uint8Array([0, 1, 2]));
  console.error("written file2");
  await Deno.remove(file1);
  console.error("removed file1");
  setTimeout(() => {
    watcher.close();
    console.error("closed watcher!");
  }, 750);
  await captureEvents(watcher, 3);
});
