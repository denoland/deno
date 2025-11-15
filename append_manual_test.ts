import fs from "node:fs/promises";

console.log("Manual append test starting...");

const file = "manual_append.txt";
await Deno.writeTextFile(file, "Hello");

const fh = await fs.open(file, "a+");
await fh.appendFile(" World");
await fh.close();

const result = await Deno.readTextFile(file);
console.log("File content:", result);

if (result !== "Hello World") {
  throw new Error("appendFile failed manually");
}

console.log("Manual appendFile test passed!");
