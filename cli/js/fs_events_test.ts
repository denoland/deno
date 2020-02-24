// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert } from "./test_util.ts";

// TODO(ry) Add more tests to specify format.

testPerm({ read: false }, function fsEventsPermissions() {
  let thrown = false;
  try {
    Deno.fsEvents(".");
  } catch (err) {
    assert(err instanceof Deno.errors.PermissionDenied);
    thrown = true;
  }
  assert(thrown);
});

async function getTwoEvents(
  iter: AsyncIterableIterator<Deno.FsEvent>
): Promise<Deno.FsEvent[]> {
  const events = [];
  for await (const event of iter) {
    console.log(">>>> event", event);
    events.push(event);
    if (events.length > 2) break;
  }
  return events;
}

testPerm({ read: true, write: true }, async function fsEventsBasic(): Promise<
  void
> {
  const testDir = await Deno.makeTempDir();
  const iter = Deno.fsEvents(testDir);

  // Asynchornously capture two fs events.
  const eventsPromise = getTwoEvents(iter);

  // Make some random file system activity.
  const file1 = testDir + "/file1.txt";
  const file2 = testDir + "/file2.txt";
  Deno.writeFileSync(file1, new Uint8Array([0, 1, 2]));
  Deno.writeFileSync(file2, new Uint8Array([0, 1, 2]));

  // We should have gotten two fs events.
  const events = await eventsPromise;
  console.log("events", events);
  assert(events.length >= 2);
  assert(events[0].kind == "create");
  assert(events[0].paths[0].includes(testDir));
  assert(events[1].kind == "create" || events[1].kind == "modify");
  assert(events[1].paths[0].includes(testDir));
});
