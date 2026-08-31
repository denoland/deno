// Regression test for https://github.com/denoland/deno/issues/22504
// When each pending lock wait occupied a thread of the tokio blocking
// threadpool, enough concurrent lock waiters would exhaust the pool (32 threads
// on unix, 4 * available_parallelism() on Windows) and deadlock, because the
// current lock holder could no longer run the fs operations it needed in order
// to release the lock.

const COUNTER_FILE = "./test.counter";
await Deno.writeTextFile(COUNTER_FILE, "0");

async function incrementCounter() {
  // Open with write access so the handle can be locked on Windows, where
  // LockFileEx requires GENERIC_READ/GENERIC_WRITE (an append-only handle
  // would be rejected with ERROR_ACCESS_DENIED).
  const lockFile = await Deno.open("./test.lock", {
    read: true,
    write: true,
    create: true,
  });

  await lockFile.lock(true);

  // These file operations would deadlock with the old implementation because
  // every blocking pool thread was occupied by a pending flock() call.
  const counter = +(await Deno.readTextFile(COUNTER_FILE));
  await Deno.writeTextFile(COUNTER_FILE, (counter + 1).toString());

  await lockFile.unlock();
  lockFile.close();

  return counter;
}

// 50 concurrent lock acquisitions — well above the smallest blocking pool
const promises = [];
for (let i = 0; i < 50; i++) {
  promises.push(incrementCounter());
}

await Promise.all(promises);

const finalCount = +(await Deno.readTextFile(COUNTER_FILE));
console.log(`final count: ${finalCount}`);
