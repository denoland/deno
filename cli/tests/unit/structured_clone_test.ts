import { assert, assertEquals } from "./test_util.ts";

// Basic tests for the structured clone algorithm. Mainly tests TypeScript
// typings. Actual functionality is tested in WPT.

Deno.test("self.structuredClone", async () => {
  const arrayOriginal = ["hello world"];
  const channelOriginal = new MessageChannel();
  const [arrayCloned, portTransferred] = self
    .structuredClone([arrayOriginal, channelOriginal.port2], {
      transfer: [channelOriginal.port2],
    });
  assert(arrayOriginal !== arrayCloned); // not the same identity
  assertEquals(arrayCloned, arrayOriginal); // but same value
  channelOriginal.port1.postMessage("1");
  await new Promise((resolve) => portTransferred.onmessage = () => resolve(1));
  channelOriginal.port1.close();
  portTransferred.close();
});
