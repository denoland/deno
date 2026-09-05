// A malformed policy does not prevent boot, but fails every fetch closed
// with the parse error instead of proceeding without the policy.

console.log("booted");

try {
  // The policy failure is raised in op_fetch before any connection attempt,
  // so this URL is never dialed.
  await fetch("http://localhost:5555/");
  console.log("fetch unexpectedly succeeded");
} catch (e) {
  console.log("fetch failed:", (e as Error).message);
}
