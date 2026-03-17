// Regression test for https://github.com/denoland/deno/issues/22504
// With the old blocking flock() implementation, 33+ concurrent lock waiters
// would exhaust the tokio blocking threadpool (32 threads) and deadlock.

const COUNTER_FILE = "./test.counter";
await Deno.writeTextFile(COUNTER_FILE, "0");

async function incrementCounter() {
  const lockFile = await Deno.open("./test.lock", {
    append: true,
    create: true,
  });

  await lockFile.lock(true);

  // These file operations would deadlock with the old implementation
  // because all 32 blocking threads were occupied by pending flock() calls.
  const counter = +(await Deno.readTextFile(COUNTER_FILE));
  await Deno.writeTextFile(COUNTER_FILE, (counter + 1).toString());

  await lockFile.unlock();
  lockFile.close();

  return counter;
}

// 50 concurrent lock acquisitions — well above the old 32-thread limit
const promises = [];
for (let i = 0; i < 50; i++) {
  promises.push(incrementCounter());
}

await Promise.all(promises);

const finalCount = +(await Deno.readTextFile(COUNTER_FILE));
console.log(`final count: ${finalCount}`);

// Clean up
try {
  Deno.removeSync("./test.lock");
  Deno.removeSync(COUNTER_FILE);
} catch {
  // ignore
}
