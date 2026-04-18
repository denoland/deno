#!/usr/bin/env node

const hasUnsafeProto = Object.hasOwn(Object.prototype, "__proto__");
if (hasUnsafeProto) {
  console.log("unsafe proto enabled");
} else {
  console.log("unsafe proto disabled");
}
