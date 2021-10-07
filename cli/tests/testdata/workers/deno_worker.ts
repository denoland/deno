import { assert } from "../../../../test_util/std/testing/asserts.ts";

onmessage = function (e) {
  if (typeof self.Deno === "undefined") {
    throw new Error("Deno namespace not available in worker");
  }

  assert(!Object.isFrozen(self.Deno));

  const desc = Object.getOwnPropertyDescriptor(self, "Deno");
  assert(desc);
  assert(desc.configurable);
  assert(!desc.writable);

  postMessage(e.data);
};
