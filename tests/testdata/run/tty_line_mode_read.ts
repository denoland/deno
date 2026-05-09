// Regression test for Windows line-mode TTY reading.
// Verifies that ReadConsoleW on a worker thread correctly delivers
// line input back to the JS readline layer. This exercises the
// threaded line-mode read path (matching libuv's
// uv_tty_line_read_thread).
import process from "node:process";
import { createInterface } from "node:readline";

function ask(question: string): Promise<string> {
  return new Promise((resolve) => {
    const rl = createInterface({
      input: process.stdin,
      output: process.stdout,
      terminal: true,
    });
    rl.question(question, (answer) => {
      rl.close();
      resolve(answer);
    });
  });
}

const a1 = await ask("Name? ");
console.log("Hello,", a1);
const a2 = await ask("Color? ");
console.log("You like", a2);
