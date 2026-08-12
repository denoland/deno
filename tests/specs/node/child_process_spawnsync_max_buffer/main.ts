// Regression test for https://github.com/denoland/deno/issues/34045 —
// `child_process.spawnSync` must terminate the child once stdout/stderr
// exceeds `maxBuffer`, instead of hanging while the child writes forever.
import { spawnSync } from "node:child_process";

const child = `
// Write much more than the maxBuffer below so the limit is hit quickly.
const chunk = "x".repeat(64 * 1024);
while (true) {
  Deno.stdout.writeSync(new TextEncoder().encode(chunk));
}
`;

const start = Date.now();
const result = spawnSync(Deno.execPath(), ["eval", child], {
  maxBuffer: 32 * 1024,
  // Enforce an upper bound so the test fails loudly rather than hanging if
  // the maxBuffer enforcement regresses.
  timeout: 15_000,
});
const elapsedMs = Date.now() - start;

if (result.error === undefined || result.error === null) {
  throw new Error(
    `expected an error to be set, got: ${JSON.stringify(result)}`,
  );
}
// deno-lint-ignore no-explicit-any
const code = (result.error as any).code;
if (code !== "ENOBUFS") {
  throw new Error(
    `expected error.code === "ENOBUFS", got: ${code} (elapsed=${elapsedMs}ms)`,
  );
}
if (result.status !== null) {
  throw new Error(`expected status === null, got: ${result.status}`);
}
if (typeof result.signal !== "string" || result.signal.length === 0) {
  throw new Error(`expected a kill signal, got: ${result.signal}`);
}
if (typeof result.pid !== "number" || result.pid <= 0) {
  throw new Error(`expected a valid pid, got: ${result.pid}`);
}
// The child should be killed quickly after the limit is hit. Anything close
// to the timeout means we hung waiting for the child.
if (elapsedMs >= 14_000) {
  throw new Error(
    `spawnSync took ${elapsedMs}ms — looks like the child was not killed promptly`,
  );
}
console.log("ok");
