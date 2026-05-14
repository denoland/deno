// Minimal reproduction: consecutive readline prompts on the same stdin/stdout
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

const a1 = await ask("Q1? ");
console.log("A1:", a1);
const a2 = await ask("Q2? ");
console.log("A2:", a2);
