import chalk from "npm:chalk@5";

export function paint(s: string): string {
  return chalk.red(s);
}
