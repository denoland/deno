// Copyright 2018-2026 the Deno authors. MIT license.

try {
  Deno.statSync(
    "./bins/bin/.lifecycle-scripts-simple/node_modules/@denotest/lifecycle-scripts-simple/message.js",
  );
  throw new Error("The lifecycle script should not have run");
} catch (error) {
  if (!(error instanceof Deno.errors.NotFound)) throw error;
}
