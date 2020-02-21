// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert } from "./test_util.ts";

// TODO(ry) Add more tests to specify format.

testPerm({ read: false }, function fsEventsPermissions() {
  let thrown = false;
  try {
    Deno.fsEvents(".");
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

testPerm({ read: true, write: true }, async function fsEventsBasic(): Promise<
  void
> {
  const testDir = await Deno.makeTempDir();
  const events: Deno.FsEvent[] = [];
  const iter = Deno.fsEvents(testDir);
  (async (): Promise<void> => {
    for await (const event of iter) {
      console.log(">>>> event", event);
      events.push(event);
      if (events.length > 2) break;
    }
  })();

  const file1 = testDir + "/file1.txt";
  const file2 = testDir + "/file2.txt";

  Deno.writeFileSync(file1, new Uint8Array([0, 1, 2]));
  Deno.writeFileSync(file2, new Uint8Array([0, 1, 2]));
  await delay(100);
  console.log("events", events);
  assert(events.length >= 2);
  assert(events[0].kind == "create");
  assert(events[0].paths[0].includes(testDir));
  assert(events[1].kind == "create" || events[1].kind == "modify");
  assert(events[1].paths[0].includes(testDir));
});

Deno.runTests();
