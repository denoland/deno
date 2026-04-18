import { spawn } from "node:child_process";

// Regression test: calling unref() on child process stdio streams should
// allow the event loop to exit. Previously, Pipe.unref() did not propagate
// to the underlying StreamResource, so the event loop would stay alive
// indefinitely (causing hangs in tools like esbuild that spawn persistent
// child processes and unref their stdio).

// Spawn a long-lived child process (cat will block reading stdin forever)
const child = spawn("cat", [], { stdio: ["pipe", "pipe", "inherit"] });

// Read stdout data (there won't be any, but set up the listener)
child.stdout!.on("data", () => {});

// Unref everything — this should allow the parent process to exit
child.unref();
child.stdin!.unref();
child.stdout!.unref();

console.log("unref succeeded, process should exit");

// If the fix works, the event loop has nothing keeping it alive and exits.
// If broken, this process hangs forever.
