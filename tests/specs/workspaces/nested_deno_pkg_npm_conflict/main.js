import { sum } from "@denotest/add";

console.log(sum(2, 2));

console.log(
  JSON.parse(Deno.readTextFileSync(
    new URL("node_modules/@denotest/add/package.json", import.meta.url),
  )).version,
);
