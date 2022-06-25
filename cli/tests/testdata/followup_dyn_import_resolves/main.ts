// https://github.com/denoland/deno/issues/14726

// Any dynamic modules that are only pending on a TLA import should be resolved
// in the same event loop iteration as the imported module.

// Long-running timer so the event loop doesn't have a next iteration for a
// while.
setTimeout(() => {}, 24 * 60 * 60 * 1000);

await import("./sub1.ts");

// If we reach here, the test is passed.
console.log("Done.");
Deno.exit();
