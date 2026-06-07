import { createInterface } from "node:readline";

createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: true,
}).close();
console.log("success!");
