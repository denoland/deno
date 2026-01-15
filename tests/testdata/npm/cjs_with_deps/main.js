import chalk from "npm:chalk@4";
import { expect } from "npm:chai@4.3";

console.log(chalk.green("chalk cjs loads"));

const timeout = setTimeout(() => {}, 0);
expect(timeout).to.be.a("number");
clearTimeout(timeout);

const interval = setInterval(() => {}, 100);
expect(interval).to.be.a("number");
clearInterval(interval);
