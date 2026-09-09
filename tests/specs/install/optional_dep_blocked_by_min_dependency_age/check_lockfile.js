// The blocked optional dep must be absent from the lockfile while the sibling
// that resolved cleanly is still recorded. Asserting the two facts directly
// rather than dumping the whole lockfile keeps this immune to unrelated
// lockfile format churn.
const lock = Deno.readTextFileSync("deno.lock");
console.log("linux-x64:", lock.includes("napi-optional-platform-linux-x64"));
console.log(
  "android-arm64:",
  lock.includes("napi-optional-platform-android-arm64"),
);
