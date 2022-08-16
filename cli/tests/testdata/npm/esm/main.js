import chalk from "npm:chalk@5";

console.log(chalk.green("chalk esm loads"));

export function test(value) {
  return chalk.red(value);
}
