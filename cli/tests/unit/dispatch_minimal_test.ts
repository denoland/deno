import {
  assert,
  assertEquals,
  assertMatch,
  unitTest,
  unreachable,
} from "./test_util.ts";

const readErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(.*dispatch_minimal\\.ts:.*\\)
    at Object.sendAsyncMinimal \\(.*dispatch_minimal\\.ts:.*\\)
    at async Object\\.read \\(.*io\\.ts:.*\\).*$`,
  "ms"
);

unitTest(async function sendAsyncStackTrace(): Promise<void> {
  const buf = new Uint8Array(10);
  const rid = 10;
  try {
    await Deno.read(rid, buf);
    unreachable();
  } catch (error) {
    assertMatch(error.stack, readErrorStackPattern);
  }
});

/* eslint-disable @typescript-eslint/no-namespace, @typescript-eslint/no-explicit-any,no-var */
declare global {
  namespace Deno {
    var core: any;
  }
}
/* eslint-enable */

unitTest(function malformedMinimalControlBuffer(): void {
  const readOpId = Deno.core.ops()["op_read"];
  const res = Deno.core.send(readOpId, new Uint8Array([1, 2, 3, 4, 5]));
  const header = res.slice(0, 12);
  const buf32 = new Int32Array(
    header.buffer,
    header.byteOffset,
    header.byteLength / 4
  );
  const arg = buf32[1];
  const message = new TextDecoder().decode(res.slice(12)).trim();
  assert(arg < 0);
  assertEquals(message, "Unparsable control buffer");
});
