if (typeof self !== "undefined") {
  throw new Error("self should be undefined in node mode");
}

if ("self" in globalThis) {
  throw new Error("globalThis.self should not exist in node mode");
}

console.log("ok");
