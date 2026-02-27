// Copyright 2018-2025 the Deno authors. MIT license.

// Test that multiple dynamic imports of a module with pending TLA
// all resolve to the same module instance without throwing
// "Cannot access 'default' before initialization"

const imports = [];

// Start multiple imports while TLA is pending
for (let i = 0; i < 5; i++) {
  imports.push(import("./tla_module.js"));
  // Small delay between imports to ensure they overlap with pending TLA
  await new Promise((r) => setTimeout(r, 10));
}

const results = await Promise.all(imports);

// Verify all imports resolved to the same module instance
const first = results[0];
for (let i = 1; i < results.length; i++) {
  if (results[i] !== first) {
    console.error("ERROR: Got different module instances");
    throw new Error("Got different module instances");
  }
}

console.log("All imports resolved to same instance");
console.log("Default export:", first.default);
