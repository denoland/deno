// Test that child_process stdout pipe works correctly through Pipe handles.
// The child writes to stdout, which flows through a Pipe with FdStreamBase.
import { spawn } from "node:child_process";

const child = spawn(Deno.execPath(), [
  "eval",
  'process.stdout.write("hello from child pipe");',
]);

let data = "";
child.stdout!.on("data", (chunk: Buffer) => {
  data += chunk.toString();
});
child.on("close", (code: number) => {
  console.log("child exited:", code);
  console.log("received:", data);
});
