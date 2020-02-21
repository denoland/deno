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
  const {
    kind: kind0,
    paths: [p0]
  } = events[0];
  const {
    kind: kind1,
    paths: [p1]
  } = events[1];
  /*
  assert(kind0 == "create");
  assert(p0.includes(testDir));
  assert(kind1 == "create" || kind1 == "modify");
  assert(p1.includes(testDir));
  */
});

Deno.runTests();
