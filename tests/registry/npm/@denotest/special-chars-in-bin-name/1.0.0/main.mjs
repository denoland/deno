import process from "node:process";

for (const arg of process.argv.slice(2)) {
  console.log(arg);
}
