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
    "regular/empty",
    "contained-link/missing",
    "contained-link/empty",
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

for (
  const specifier of [
    "outside-link/found",
    "outside-link/missing",
    "outside-link",
    "dangling-link/missing",
  ]
) {
  try {
    require(specifier);
    throw new Error(`Expected ${specifier} to be unavailable`);
  } catch (error) {
    assertEquals(error.name, "NotCapable");
  }
}

console.log("ok");
