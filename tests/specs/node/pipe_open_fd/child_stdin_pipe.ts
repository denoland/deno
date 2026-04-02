// Test that child_process stdin pipe works correctly through Pipe handles.
// We write to the child's stdin, child reads and echoes it back via stdout.
import { spawn } from "node:child_process";

const child = spawn(Deno.execPath(), [
  "eval",
  `
  const chunks: Buffer[] = [];
  process.stdin.on("data", (chunk: Buffer) => chunks.push(chunk));
  process.stdin.on("end", () => {
    process.stdout.write("echo: " + Buffer.concat(chunks).toString());
  });
  `,
]);

child.stdin!.write("data through pipe");
child.stdin!.end();

let data = "";
child.stdout!.on("data", (chunk: Buffer) => {
  data += chunk.toString();
});
child.on("close", (code: number) => {
  console.log("child exited:", code);
  console.log("received:", data);
});
