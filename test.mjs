import process from "node:process";
import readline from "node:readline";

const input = readline.createInterface({
  input: process.stdin,
});

process.stdin.unref();
