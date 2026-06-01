import { fork } from "node:child_process";

// Regression test for https://github.com/denoland/deno/issues/26304
// In a compiled binary, fork() must run the embedded `child.js` module rather
// than re-running this entrypoint. The child echoes a message back over IPC.
const child = fork("./child.js");
child.on("message", (msg) => {
  console.log("parent got:", msg.reply);
  child.kill();
  Deno.exit(0);
});
child.send({ ask: "ping" });
