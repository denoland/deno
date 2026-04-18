// Regression test for https://github.com/denoland/deno/issues/32997
// Simulates interactive prompts that toggle raw mode (like vite create,
// @clack/prompts, etc.) to verify that console mode is properly restored
// between prompts and stdin continues to work.
import process from "node:process";
import { createInterface } from "node:readline";

function ask(question: string): Promise<string> {
  return new Promise((resolve) => {
    const rl = createInterface({
      input: process.stdin,
      output: process.stdout,
      terminal: true,
    });
    // Enable raw mode to simulate interactive picker behavior
    if (process.stdin.isTTY) {
      process.stdin.setRawMode(true);
    }
    rl.question(question, (answer) => {
      if (process.stdin.isTTY) {
        process.stdin.setRawMode(false);
      }
      rl.close();
      resolve(answer);
    });
  });
}

const a1 = await ask("Step1? ");
console.log("Got1:", a1);
const a2 = await ask("Step2? ");
console.log("Got2:", a2);
const a3 = await ask("Step3? ");
console.log("Got3:", a3);
