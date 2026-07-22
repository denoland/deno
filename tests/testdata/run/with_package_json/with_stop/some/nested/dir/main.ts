// This import should fail, because `package.json` is not discovered, as we're
// stopping the discovery when encountering `deno.json`.
import chalk from "chalk";

console.log("ok");
console.log(chalk);
