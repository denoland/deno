// Regression test for https://github.com/denoland/deno/issues/33085
// `process.send(buf); process.exit(0)` must reliably deliver the message to
// the parent. Previously the IPC write went through Tokio's async readiness
// and was never polled before the runtime terminated, so the parent saw the
// child exit without ever receiving the message (flaky).

const { fork } = require("child_process");
const path = require("path");

const child = fork(path.join(__dirname, "child.cjs"));

child.on("message", (m) => {
  console.log("got message length:", m.length || (m && m.byteLength) || "?");
});
child.on("exit", (code) => {
  console.log("child exited with code:", code);
});
