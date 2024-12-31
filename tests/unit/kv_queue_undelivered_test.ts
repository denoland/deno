// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals } from "./test_util.ts";

const sleep = (time: number) => new Promise((r) => setTimeout(r, time));

// TODO(igorzi): https://github.com/denoland/deno/issues/21437
// let isCI: boolean;
// try {
//   isCI = Deno.env.get("CI") !== undefined;
// } catch {
//   isCI = true;
// }

function queueTest(name: string, fn: (db: Deno.Kv) => Promise<void>) {
  // TODO(igorzi): https://github.com/denoland/deno/issues/21437
  Deno.test.ignore({
    name,
    // https://github.com/denoland/deno/issues/18363
    // ignore: Deno.build.os === "darwin" && isCI,
    async fn() {
      const db: Deno.Kv = await Deno.openKv(
        ":memory:",
      );
      await fn(db);
    },
  });
}

async function collect<T>(
  iter: Deno.KvListIterator<T>,
): Promise<Deno.KvEntry<T>[]> {
  const entries: Deno.KvEntry<T>[] = [];
  for await (const entry of iter) {
    entries.push(entry);
  }
  return entries;
}

queueTest("queue with undelivered", async (db) => {
  const listener = db.listenQueue((_msg) => {
    throw new TypeError("dequeue error");
  });
  try {
    await db.enqueue("test", {
      keysIfUndelivered: [["queue_failed", "a"], ["queue_failed", "b"]],
      backoffSchedule: [10, 20],
    });
    await sleep(3000);
    const undelivered = await collect(db.list({ prefix: ["queue_failed"] }));
    assertEquals(undelivered.length, 2);
    assertEquals(undelivered[0].key, ["queue_failed", "a"]);
    assertEquals(undelivered[0].value, "test");
    assertEquals(undelivered[1].key, ["queue_failed", "b"]);
    assertEquals(undelivered[1].value, "test");
  } finally {
    db.close();
    await listener;
  }
});
