import chalk from "chalk";
import { getSubPathKind } from "@denotest/subpath/main.mjs";

console.log(chalk.green("chalk import map loads"));

export function test(value) {
  return chalk.red(value);
}

console.log(getSubPathKind());
