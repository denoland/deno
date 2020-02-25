import { assert, assertEquals, assertMatch, unreachable } from "./test_util.ts";

const readErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(.*dispatch_minimal\\.ts:.*\\)
    at Object.sendAsyncMinimal \\(.*dispatch_minimal\\.ts:.*\\)
    at async Object\\.read \\(.*files\\.ts:.*\\).*$`,
  "ms"
);

test(async function sendAsyncStackTrace(): Promise<void> {
  const buf = new Uint8Array(10);
  const rid = 10;
  try {
    await Deno.read(rid, buf);
    unreachable();
  } catch (error) {
    assertMatch(error.stack, readErrorStackPattern);
  }
});
