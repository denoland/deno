#!/usr/bin/env node

let hasUnsafeProto;
try {
  // When the `__proto__` accessor is disabled this throws; with
  // --unstable-unsafe-proto the native accessor returns the prototype.
  ({}).__proto__;
  hasUnsafeProto = true;
} catch {
  hasUnsafeProto = false;
}
if (hasUnsafeProto) {
  console.log("unsafe proto enabled");
} else {
  console.log("unsafe proto disabled");
}
