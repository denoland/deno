import { createRequire } from "node:module";

function assertEquals(actual, expected) {
  if (actual !== expected) {
    throw new Error(`Expected ${expected}, got ${actual}`);
  }
}

const require = createRequire(import.meta.url);

assertEquals(require("regular/found"), "regular");
assertEquals(require("contained-link/found"), "contained");

for (
  const specifier of [
    "regular/missing",
    "contained-link/missing",
    "contained-dangling-link/missing",
  ]
) {
  try {
    require(specifier);
    throw new Error(`Expected ${specifier} to be missing`);
  } catch (error) {
    assertEquals(error.code, "MODULE_NOT_FOUND");
  }
}

console.log("ok");
