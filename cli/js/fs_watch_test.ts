// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert } from "./test_util.ts";

// TODO(ry) Add more tests to specify format.

testPerm({ read: false, write: false }, function fsWatchPermissions() {
  let thrown = false;
  try {
    Deno.watch(".");
  } catch (e) {
    assert(e.kind == Deno.ErrorKind.PermissionDenied);
    thrown = true;
  }
  assert(thrown);
});

function delay(ms: number): Promise<void> {
  return new Promise(r => {
    setTimeout(() => r(), ms);
  });
}

testPerm({ read: true, write: true }, async function fsWatcherBasic(): Promise<
  void
> {
  const events: Deno.FsEvent[] = [];
  async function captureEvents(watcher: Deno.FsWatcher): Promise<void> {
    for await (const event of watcher) {
      console.log("event", event);
      events.push(event);
      console.error("got event!", event);
    }
  }

  const testDir = await Deno.makeTempDir();
  const file1 = testDir + "/file1.txt";
  const file2 = testDir + "/file2.txt";

  const watcher = Deno.watch(testDir, { recursive: true });
  captureEvents(watcher);

  Deno.writeFileSync(file1, new Uint8Array([0, 1, 2]));
  Deno.writeFileSync(file2, new Uint8Array([0, 1, 2]));

  await delay(1000);

  console.log("events", events);
  assert(events.length >= 2);
  watcher.close();
});

Deno.runTests();
