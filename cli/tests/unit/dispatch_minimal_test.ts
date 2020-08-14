import {
  assert,
  assertEquals,
  assertMatch,
  unitTest,
  unreachable,
} from "./test_util.ts";

const readErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(.*dispatch_minimal\\.js:.*\\)
    at sendAsync \\(.*dispatch_minimal\\.js:.*\\)
    at async Object\\.read \\(.*io\\.js:.*\\).*$`,
  "ms",
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

declare global {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace Deno {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    var core: any; // eslint-disable-line no-var
  }
}

unitTest(function malformedMinimalControlBuffer(): void {
  const readOpId = Deno.core.ops()["op_read"];
  const res = Deno.core.send(readOpId, new Uint8Array([1, 2, 3, 4, 5]));
  const header = res.slice(0, 12);
  const buf32 = new Int32Array(
    header.buffer,
    header.byteOffset,
    header.byteLength / 4,
  );
  const arg = buf32[1];
  const codeAndMessage = new TextDecoder().decode(res.slice(12)).trim();
  assert(arg < 0);
  assertEquals(codeAndMessage, "TypeErrorUnparsable control buffer");
});
