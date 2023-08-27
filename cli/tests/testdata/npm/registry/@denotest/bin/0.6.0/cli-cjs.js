const process = require("process");

for (const arg of process.argv.slice(2)) {
  console.log(arg);
}
