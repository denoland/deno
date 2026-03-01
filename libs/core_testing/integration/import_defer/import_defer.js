// Copyright 2018-2026 the Deno authors. MIT license.

// Test for TC39 proposal: Deferred Module Evaluation
// https://github.com/tc39/proposal-defer-import-eval
//
// The `import defer` syntax allows loading a module without immediately
// executing it. The module is executed synchronously when any property
// on the namespace is first accessed.
//
console.log("before import defer");

// Static import defer syntax - module is loaded but not executed
import defer * as deferred from "./deferred.js";

console.log("after import defer, before access");

// First property access triggers module evaluation
console.log(`value: ${deferred.value}`);

console.log("after first access");

// Subsequent accesses use the already-evaluated module
console.log(`add: ${deferred.add(1, 2)}`);

console.log("done");
