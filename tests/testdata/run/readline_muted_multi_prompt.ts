// Reproduce @inquirer/prompts pattern: MuteStream piped to stdout
import process from "node:process";
import { createInterface } from "node:readline";
import { PassThrough } from "node:stream";

function ask(question: string): Promise<string> {
  return new Promise((resolve) => {
    const output = new PassThrough();
    output.pipe(process.stdout);

    const rl = createInterface({
      input: process.stdin,
      output: output,
      terminal: true,
    });

    rl.question(question, (answer) => {
      rl.close();
      output.unpipe(process.stdout);
      output.end();
      resolve(answer);
    });
  });
}

const a1 = await ask("Q1? ");
console.log("A1:", a1);
const a2 = await ask("Q2? ");
console.log("A2:", a2);
