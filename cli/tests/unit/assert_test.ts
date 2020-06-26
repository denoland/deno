import { unitTest } from "./test_util.ts";

unitTest(function denoAssert() {
  if (!("assert" in Deno)) {
    throw new Error("Deno.assert missing");
  }
  if (typeof Deno.assert !== "function") {
    throw new Error("Deno.assert not a function");
  }
  Deno.assert(true);
  try {
    Deno.assert(false, "a failure");
  } catch (e) {
    if (!(e instanceof Deno.AssertionError)) {
      throw new Error("Deno.assert does not throw properly");
    }
    if (e.message !== "a failure") {
      throw new Error("Deno.assert does not properly set message");
    }
  }
});
