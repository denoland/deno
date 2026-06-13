// Regression test: when spawnSync inherits stdio, the implicit Node default
// `maxBuffer` must NOT cause the child to be moved into a new process group.
// Doing so makes `tcsetattr` (e.g. `setRawMode`) raise SIGTTOU and hang the
// parent indefinitely when stdin is a controlling TTY — see CI failures on
// `pseudo-tty/test-set-raw-mode-reset.js` after the maxBuffer change.
//
// We can't easily attach a real TTY here, but we can verify the cheaper
// invariant: spawnSync with `stdio: 'inherit'` (no piped output) returns
// promptly with status 0 instead of hanging on the runner's outer timeout.
import { spawnSync } from "node:child_process";

const start = Date.now();
const result = spawnSync(
  Deno.execPath(),
  ["eval", "console.log('child done')"],
  { stdio: "inherit", timeout: 10_000 },
);
const elapsedMs = Date.now() - start;

if (result.status !== 0) {
  throw new Error(
    `expected status === 0, got: status=${result.status} signal=${result.signal} elapsed=${elapsedMs}ms`,
  );
}
if (result.signal !== null) {
  throw new Error(`expected signal === null, got: ${result.signal}`);
}
if (elapsedMs >= 9_000) {
  throw new Error(
    `spawnSync inherit-stdio took ${elapsedMs}ms — looks like it hung`,
  );
}
console.log("ok");
