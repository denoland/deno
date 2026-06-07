#!/usr/bin/env -S node

import process from "node:process";

console.log("cli");
for (const arg of process.argv.slice(2)) {
  console.log(arg);
}
