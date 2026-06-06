// The entrypoint lives in `entry/` (its package scope), where the npm
// dependency resolves normally. It spawns a worker authored in a sibling
// `outside/` directory, which is outside this package scope. `deno compile`
// follows the `new Worker(new URL(...))` reference and pulls that source file
// into the graph; its bare npm import must still resolve against the build's
// npm snapshot. See https://github.com/denoland/deno/issues/34937.
import { getValue, setValue } from "@denotest/esm-basic";

setValue(5);
console.log("main", getValue());

const worker = new Worker(
  new URL("../outside/worker.ts", import.meta.url),
  { type: "module" },
);
worker.onmessage = (e) => {
  console.log("main got:", e.data);
  worker.terminate();
};
worker.postMessage("ping");
