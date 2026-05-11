import chalk from "chalk";
import { strip } from "./util.ts";

export function red(s: string): string {
  return chalk.red(strip(s));
}

export { strip };
