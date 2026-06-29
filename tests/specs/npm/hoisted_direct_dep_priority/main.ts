const hoisted = JSON.parse(
  Deno.readTextFileSync(
    "node_modules/@denotest/different-nested-dep-child/package.json",
  ),
);
const nested = JSON.parse(
  Deno.readTextFileSync(
    "node_modules/@denotest/needs-different-nested-dep-child-v2/node_modules/@denotest/different-nested-dep-child/package.json",
  ),
);
console.log("hoisted:", hoisted.version);
console.log("nested:", nested.version);
