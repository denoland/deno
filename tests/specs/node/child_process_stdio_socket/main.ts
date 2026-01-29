import { spawn } from "node:child_process";
import { Socket } from "node:net";

// Test that stdio streams are Socket instances (Node.js compatibility)
// Issue: https://github.com/denoland/deno/issues/31961 and https://github.com/denoland/deno/issues/25602

const cp = spawn("node", [], {
  stdio: "pipe",
});

// stdout should be a Socket instance
console.log("stdout instanceof Socket:", cp.stdout instanceof Socket);
console.log(
  "stdout has unref:",
  "unref" in cp.stdout! && typeof cp.stdout!.unref === "function"
);

// stderr should be a Socket instance
console.log("stderr instanceof Socket:", cp.stderr instanceof Socket);
console.log(
  "stderr has unref:",
  "unref" in cp.stderr! && typeof cp.stderr!.unref === "function"
);

// stdin should be a Socket instance
console.log("stdin instanceof Socket:", cp.stdin instanceof Socket);
console.log(
  "stdin has unref:",
  "unref" in cp.stdin! && typeof cp.stdin!.unref === "function"
);

// Verify that unref() can be called without error
cp.stdout!.unref();
cp.stderr!.unref();
cp.stdin!.unref();
console.log("unref() calls succeeded");

cp.kill();
