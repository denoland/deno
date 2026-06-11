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
  async function watchFsCloseIsIdempotent() {
    const testDir = await makeTempDir();
    const watcher = Deno.watchFs(testDir);
    const iterator = watcher[Symbol.asyncIterator]();

    watcher.close();
    watcher.close();

    assertEquals(await iterator.next(), { value: undefined, done: true });
    assertEquals(await iterator.return!(), { value: undefined, done: true });
    assertEquals(await iterator.next(), { value: undefined, done: true });
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

// Regression test for https://github.com/denoland/deno/issues/27742
// Closing a `Deno.FsWatcher` and creating a new one for the same path must
// not produce duplicate events on the new watcher. Before the fix, the
// shared `RecommendedWatcher` never had paths unwatched on close, so on
// Windows each closed-then-recreated watcher left behind another
// `ReadDirectoryChangesW` registration, causing N-fold duplicate events.
Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsCloseAndRecreateNoDuplicates() {
    const testDir = await makeTempDir();

    async function openClose() {
      const w = Deno.watchFs(testDir);
      const closer = setTimeout(() => w.close(), 100);
      for await (const _ of w) {
        // drain
      }
      clearTimeout(closer);
    }

    // Open and close three watchers in sequence.
    await openClose();
    await openClose();
    await openClose();

    // Now open a fourth watcher and trigger a single fs event. We must
    // only see events for that single change reach us once per real event.
    using watcher = Deno.watchFs(testDir);

    const target = testDir + "/probe.txt";
    const writePromise = (async () => {
      // Give the watcher a moment to settle before producing the event.
      await delay(100);
      Deno.writeFileSync(target, new Uint8Array([1, 2, 3]));
    })();

    // Collect events for a brief window after the write. Any event we
    // observe must match a real change to probe.txt; we then count how
    // many times each (kind, path) tuple appears. Filesystems legitimately
    // emit at most a couple of events for a single write (`create` plus
    // an optional `modify`), so seeing the same tuple 3+ times means the
    // bug is still present.
    const seen = new Map<string, number>();
    const collectPromise = (async () => {
      const start = Date.now();
      for await (const event of watcher) {
        if (event.paths.some((p) => p.endsWith("probe.txt"))) {
          const key = `${event.kind}:${event.paths.join(",")}`;
          seen.set(key, (seen.get(key) ?? 0) + 1);
        }
        if (Date.now() - start > 500) break;
      }
    })();

    await writePromise;
    // Wait long enough to receive any duplicates the buggy implementation
    // would emit.
    await delay(600);
    watcher.close();
    await collectPromise;

    for (const [key, count] of seen) {
      assert(
        count <= 2,
        `event "${key}" was emitted ${count} times; expected at most 2 ` +
          `(create + optional modify). This indicates a leaked watch from ` +
          `a previously-closed Deno.FsWatcher.`,
      );
    }
  },
);

// Regression test for https://github.com/denoland/deno/issues/11373
// A burst of concurrent file writes must not be silently lost: every file
// written below must show up in at least one event, unless the loss is
// explicitly reported via a `rescan`-flagged event.
Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsBurstDeliversAllEvents() {
    const testDir = await makeTempDir();
    using watcher = Deno.watchFs(testDir);

    const fileCount = 100;
    // Match events by file name: on macOS the events report canonicalized
    // paths (/private/var/...) while makeTempDir returns /var/... paths.
    const remaining = new Set<string>();
    for (let i = 0; i < fileCount; i++) {
      remaining.add(`file${i}.txt`);
    }

    // Escape hatch so the iteration below can't hang the test forever if
    // events do get lost.
    const deadline = setTimeout(() => watcher.close(), 20_000);
    let sawRescan = false;
    const collectPromise = (async () => {
      for await (const event of watcher) {
        if (event.flag === "rescan") {
          sawRescan = true;
          break;
        }
        for (const path of event.paths) {
          const name = path.replaceAll("\\", "/").split("/").pop()!;
          remaining.delete(name);
        }
        if (remaining.size === 0) break;
      }
    })();

    await Promise.all(
      [...remaining].map((name) =>
        Deno.writeTextFile(`${testDir}/${name}`, "content")
      ),
    );

    await collectPromise;
    clearTimeout(deadline);
    assert(
      remaining.size === 0 || sawRescan,
      `events for ${remaining.size}/${fileCount} files were silently lost`,
    );
  },
);

// Regression test for https://github.com/denoland/deno/issues/11373
// When events arrive faster than the consumer drains them and the internal
// queue overflows, the loss must be reported with a `rescan`-flagged event
// instead of being silent.
Deno.test(
  { permissions: { read: true, write: true } },
  async function watchFsQueueOverflowEmitsRescan() {
    const testDir = await makeTempDir();
    using watcher = Deno.watchFs(testDir);

    // Produce more events than the internal queue can hold (1024 entries,
    // see FS_EVENT_QUEUE_CAPACITY in runtime/ops/fs_events.rs) while the
    // consumer is not yet polling.
    for (let i = 0; i < 1500; i++) {
      Deno.writeFileSync(`${testDir}/file${i}.txt`, new Uint8Array([1]));
    }
    // Give the watcher backend time to process (and overflow on) the burst
    // before we start consuming.
    await delay(3000);

    // Escape hatch so the iteration below can't hang the test forever if
    // the rescan event never arrives.
    const deadline = setTimeout(() => watcher.close(), 20_000);
    // Don't require the rescan to be the *first* event: on a slow runner the
    // backend may not have overflowed the queue yet when polling starts, in
    // which case some regular events are delivered before the overflow
    // happens and is reported.
    let sawRescan = false;
    for await (const event of watcher) {
      if (event.flag === "rescan") {
        sawRescan = true;
        break;
      }
    }
    clearTimeout(deadline);
    assert(sawRescan, "expected a rescan event after the queue overflowed");
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
