// Copyright 2018-2026 the Deno authors. MIT license.

/**
 * This is needed to test that stack traces in extensions are correct.
 */
export function throwExceptionFromExtension() {
  innerThrowInExt();
}

function innerThrowInExt() {
  throw new Error("Failed");
}
