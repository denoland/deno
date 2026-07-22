#!/usr/bin/env node

// Detect by behavior, not presence: Deno keeps the `__proto__` accessor
// installed even when disabled (so it can capture assignments), so reading it
// is what actually distinguishes the modes — disabled returns `undefined`.
const hasUnsafeProto = ({}).__proto__ !== undefined;
if (hasUnsafeProto) {
  console.log("unsafe proto enabled");
} else {
  console.log("unsafe proto disabled");
}
