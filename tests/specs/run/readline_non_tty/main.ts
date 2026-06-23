import { createInterface } from "node:readline";
import { stdin, stdout } from "node:process";

createInterface({
  input: stdin,
  output: stdout,
  terminal: true,
}).close();
console.log("success!");
