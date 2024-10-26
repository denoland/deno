import chalk from "npm:chalk";

console.log(chalk.green("Hi"));
try {
  await import("npm:@denotest/dep-cannot-parse");
} catch (err) {
  console.log(err);
}
console.log(chalk.green("Bye"));
