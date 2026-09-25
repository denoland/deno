const messagePath =
  "./bins-denied/bin/.lifecycle-scripts-simple/node_modules/@denotest/lifecycle-scripts-simple/message.js";

try {
  await Deno.stat(messagePath);
  throw new Error("postinstall script should not have run");
} catch (err) {
  if (!(err instanceof Deno.errors.NotFound)) {
    throw err;
  }
}
