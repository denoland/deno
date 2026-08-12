import { createInterface } from "node:readline/promises";

const rl = createInterface({
  input: process.stdin,
  output: process.stdout,
});

setTimeout(() => {
  try {
    Deno.readTextFileSync("/etc/npm/config");
  } catch (err) {
    console.log(`READ_ERROR:${err instanceof Error ? err.name : err}`);
  }
}, 500);

const answer = await rl.question("Project? ");
console.log(`ANSWER:${answer}`);
rl.close();
