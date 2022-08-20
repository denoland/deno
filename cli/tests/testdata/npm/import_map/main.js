import chalk from "chalk";

console.log(chalk.green("chalk import map loads"));

export function test(value) {
  return chalk.red(value);
}
