// A non-ASCII policy value is rejected at parse time and poisons the policy.
// It must not panic the process: the operator gets the same fail-closed error
// as any other malformed policy.

console.log("booted");

try {
  await fetch("http://localhost:5555/");
  console.log("fetch unexpectedly succeeded");
} catch (e) {
  console.log("fetch failed:", (e as Error).message);
}
