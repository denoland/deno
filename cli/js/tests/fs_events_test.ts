// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

// TODO(ry) Add more tests to specify format.

unitTest({ perms: { read: false } }, function fsEventsPermissions() {
  let thrown = false;
  try {
    Deno.fsEvents(".");
  } catch (err) {
    assert(err instanceof Deno.errors.PermissionDenied);
    thrown = true;
  }
  assert(thrown);
});

unitTest({ perms: { read: true } }, function fsEventsInvalidPath() {
  let thrown = false;
  try {
    Deno.fsEvents("non-existant.file");
  } catch (err) {
    console.error(err);
    if (Deno.build.os === "win") {
      assert(
        err.message.includes(
          "Input watch path is neither a file nor a directory"
        )
      );
    } else {
      assert(err instanceof Deno.errors.NotFound);
    }
    thrown = true;
  }
  assert(thrown);
});

async function getTwoEvents(
  iter: AsyncIterableIterator<Deno.FsEvent>
): Promise<Deno.FsEvent[]> {
  const events = [];
  for await (const event of iter) {
    events.push(event);
    if (events.length > 2) break;
  }
  return events;
}

unitTest(
  { perms: { read: true, write: true } },
  async function fsEventsBasic(): Promise<void> {
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
    assert(events.length >= 2);
    assert(events[0].kind == "create");
    assert(events[0].paths[0].includes(testDir));
    assert(events[1].kind == "create" || events[1].kind == "modify");
    assert(events[1].paths[0].includes(testDir));
  }
);
