// The working directory is process-global: a chdir on any thread must be
// observable from every other thread on the next Deno.cwd() read.
const initialCwd = Deno.cwd();
const tempDir = Deno.makeTempDirSync();

const worker = new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
});

const { promise, resolve, reject } = Promise.withResolvers<void>();

worker.onmessage = (e) => {
  try {
    if (e.data.type === "chdir-done") {
      // The worker changed directory; this thread must observe it.
      if (Deno.cwd() !== e.data.cwd) {
        throw new Error(
          `main thread sees stale cwd: ${Deno.cwd()} !== ${e.data.cwd}`,
        );
      }
      console.log("main observes worker chdir");
      // Now change back on the main thread; the worker must observe it.
      Deno.chdir(initialCwd);
      worker.postMessage({ type: "verify", cwd: Deno.cwd() });
    } else if (e.data.type === "verify-done") {
      console.log("worker observes main chdir");
      resolve();
    }
  } catch (err) {
    reject(err);
  }
};
worker.onerror = (e) => reject(new Error(e.message));

worker.postMessage({ type: "chdir", dir: tempDir });
await promise;
worker.terminate();
Deno.removeSync(tempDir);
console.log("done");
