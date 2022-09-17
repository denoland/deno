import chalk from "npm:chalk@5";

console.log(chalk.green("chalk import map loads"));

export function test(value) {
  return chalk.red(value);
}
