import { spawn } from "node:child_process";

// Any long-running process works — using "cat" for simplicity
const child = spawn("cat", [], { stdio: "pipe" });

child.stdout?.on("data", () => {});
child.stderr?.on("data", () => {});
child.unref(); // Should allow the event loop to proceed, not hang the preload

console.log("preload done");
