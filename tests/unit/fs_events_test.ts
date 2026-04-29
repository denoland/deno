// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, assertThrows, delay } from "./test_util.ts";

// TODO(ry) Add more tests to specify format.

Deno.test({ permissions: { read: false } }, function watchFsPermissions() {
  assertThrows(() => {
    Deno.watchFs(".");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, function watchFsInvalidPath() {
  if (Deno.build.os === "windows") {
    assertThrows(
      () => {
        Deno.watchFs("non-existent.file");
      },
      Error,
      "Input watch path is neither a file nor a directory",
    );
  } else {
    assertThrows(() => {
      Deno.watchFs("non-existent.file");
    }, Deno.errors.NotFound);
  }
});

async function getTwoEvents(
  iter: Deno.FsWatcher,
): Promise<Deno.FsEvent[]> {
  const events = [];
  for await (const event of iter) {
    events.push(event);
    if (events.length > 2) break;
  }
  return events;
}

async function makeTempDir(): Promise<string> {
  const testDir = await Deno.makeTempDir();
  // The watcher sometimes witnesses the creation of it's own root
  // directory. Delay a bit.
  await delay(100);
  return testDir;
}

async function makeTempFile(): Promise<string> {
  const testFile = await Deno.makeTempFile();
  // The watcher sometimes witnesses the creation of it's own root
  // directory. Delay a bit.
  await delay(100);
  return testFile;
}

Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsBasic() {
    const testDir = await makeTempDir();
    const iter = Deno.watchFs(testDir);

    // Asynchronously capture two fs events.
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
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsRename() {
    const testDir = await makeTempDir();
    const watcher = Deno.watchFs(testDir);
    async function waitForRename() {
      for await (const event of watcher) {
        if (event.kind === "rename") {
          break;
        }
      }
    }
    const eventPromise = waitForRename();
    const file = testDir + "/file.txt";
    await Deno.writeTextFile(file, "hello");
    await Deno.rename(file, testDir + "/file2.txt");
    await eventPromise;
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsReturn() {
    const testDir = await makeTempDir();
    const iter = Deno.watchFs(testDir);

    // Asynchronously loop events.
    const eventsPromise = getTwoEvents(iter);

    // Close the watcher.
    await iter.return!();

    // Expect zero events.
    const events = await eventsPromise;
    assertEquals(events, []);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsClose() {
    const testDir = await makeTempDir();
    const iter = Deno.watchFs(testDir);

    // Asynchronously loop events.
    const eventsPromise = getTwoEvents(iter);

    // Close the watcher.
    iter.close();

    // Expect zero events.
    const events = await eventsPromise;
    assertEquals(events, []);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsExplicitResourceManagement() {
    let res;
    {
      const testDir = await makeTempDir();
      using iter = Deno.watchFs(testDir);

      res = iter[Symbol.asyncIterator]().next();
    }

    const { done } = await res;
    assert(done);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsExplicitResourceManagementManualClose() {
    const testDir = await makeTempDir();
    using iter = Deno.watchFs(testDir);

    const res = iter[Symbol.asyncIterator]().next();

    iter.close();
    const { done } = await res;
    assert(done);
  },
);

// Regression test for https://github.com/denoland/deno/issues/27558
// Removing a file outside the watched directory should not produce events.
Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsNoSpuriousEventsFromSiblingDir() {
    const parentDir = await makeTempDir();
    const watchedDir = parentDir + "/watched";
    const otherDir = parentDir + "/other";
    Deno.mkdirSync(watchedDir);
    Deno.mkdirSync(otherDir);
    await delay(100);

    using watcher = Deno.watchFs(watchedDir);

    // Create and immediately remove a file in the sibling directory.
    // Before the fix, this would send a spurious "remove" event to
    // watchers on watchedDir because the is_file_removed fallback had
    // no path filtering.
    const otherFile = otherDir + "/temp.txt";
    Deno.writeFileSync(otherFile, new Uint8Array([1, 2, 3]));
    Deno.removeSync(otherFile);

    // Now create a real file in the watched directory so we have
    // something to observe.
    await delay(100);
    const realFile = watchedDir + "/real.txt";
    Deno.writeFileSync(realFile, new Uint8Array([1, 2, 3]));

    // Collect events -- the first meaningful event should be for real.txt,
    // not a spurious remove for temp.txt.
    for await (const event of watcher) {
      // We should never see events for files in otherDir
      for (const path of event.paths) {
        assert(
          !path.includes("other"),
          `Received spurious event for file outside watched dir: ${path}`,
        );
      }
      // Once we see the expected create, we're done
      if (event.kind === "create" || event.kind === "modify") {
        assert(event.paths[0].includes("real.txt"));
        break;
      }
    }
  },
);

// Regression test for https://github.com/denoland/deno/issues/32000
// Watching a relative path like "./" must produce event paths free of
// embedded "./" segments. Previously notify joined "./" with the relative
// portion of each event, so callers got `<cwd>/./sub/file` instead of
// `<cwd>/sub/file`.
Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsRelativePathNoCurDirSegment() {
    const testDir = await makeTempDir();
    const subDir = testDir + "/sub";
    Deno.mkdirSync(subDir);
    await delay(100);

    const originalCwd = Deno.cwd();
    Deno.chdir(testDir);
    try {
      using watcher = Deno.watchFs("./");

      const target = subDir + "/file.txt";
      const writePromise = (async () => {
        await delay(50);
        Deno.writeFileSync(target, new Uint8Array([1, 2, 3]));
      })();

      for await (const event of watcher) {
        for (const path of event.paths) {
          const sep = Deno.build.os === "windows" ? "\\" : "/";
          assert(
            !path.includes(`${sep}.${sep}`),
            `event path should not contain "${sep}.${sep}": ${path}`,
          );
          assert(
            !path.endsWith(`${sep}.`),
            `event path should not end with "${sep}.": ${path}`,
          );
        }
        if (event.paths.some((p) => p.endsWith("file.txt"))) {
          break;
        }
      }
      await writePromise;
    } finally {
      Deno.chdir(originalCwd);
    }
  },
);

// On macOS, FSEvents does not reliably emit remove events for individually
// watched files. The previous implementation masked this by forwarding
// unrelated events for any non-existent file to all watchers (the bug
// behind #27558). Skip on macOS until the notify crate or our watcher
// can detect removals of individually watched files on this platform.
Deno.test(
  {
    permissions: { read: true, write: true },
    ignore: Deno.build.os === "darwin",
  },
  async function watchFsRemove() {
    const testFile = await makeTempFile();
    using watcher = Deno.watchFs(testFile);
    async function waitForRemove() {
      for await (const event of watcher) {
        if (event.kind === "remove") {
          return event;
        }
      }
    }
    const eventPromise = waitForRemove();

    await Deno.remove(testFile);

    // Expect zero events.
    const event = await eventPromise;
    assertEquals(event!.kind, "remove");
  },
);
