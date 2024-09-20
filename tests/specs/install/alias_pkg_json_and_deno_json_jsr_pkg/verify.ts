import * as mod from "alias";

const data = JSON.parse(
  Deno.readTextFileSync(
    new URL("./node_modules/alias/package.json", import.meta.url),
  ),
);

// this should just setup the npm package anyway, even though the alias
// will resolve to the jsr package
console.log(data.name);

console.log(mod);
