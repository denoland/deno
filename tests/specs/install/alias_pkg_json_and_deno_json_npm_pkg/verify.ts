import * as mod from "alias";

const data = JSON.parse(
  Deno.readTextFileSync(
    new URL("./node_modules/alias/package.json", import.meta.url),
  ),
);

console.log(data.name);
console.log(mod);
