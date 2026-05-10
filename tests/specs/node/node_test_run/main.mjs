import { run } from "node:test";

// `run()` should return a `TestsStream` (a Readable). Iterating it should
// finish without errors. Deno does not run tests programmatically through
// `run()`, so it returns an empty stream that ends immediately.
for (let i = 0; i < 5; i++) {
  const stream = await run();
  for await (const _event of stream) {
    // no-op
  }
}

// Calling without awaiting and consuming should also be safe.
const s = run({ files: ["nonexistent.js"] });
s.on("end", () => {
  console.log("ended");
});
s.resume();
