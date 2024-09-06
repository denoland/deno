import { add } from "@denotest/add";

console.log(add(1, 2));

console.log(
  JSON.parse(Deno.readTextFileSync(
    new URL("node_modules/@denotest/add/package.json", import.meta.url),
  )).version,
);
