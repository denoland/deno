import chalk from "npm:chalk@5";

console.log(chalk.green("Starting..."));
// non-analyzable
const importName = "./other.ts";
await import(importName);
console.log(chalk.green("Finished..."));
