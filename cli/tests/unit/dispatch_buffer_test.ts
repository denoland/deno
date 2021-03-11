import {
  assert,
  assertEquals,
  assertMatch,
  unitTest,
  unreachable,
} from "./test_util.ts";

const readErrorStackPattern = new RegExp(
  `^.*
    at handleError \\(.*10_dispatch_minimal\\.js:.*\\)
    at promiseBufferRouter \\(.*10_dispatch_minimal\\.js:.*\\)
    at Array.<anonymous> \\(.*10_dispatch_minimal\\.js:.*\\).*$`,
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
  // deno-lint-ignore no-namespace
  namespace Deno {
    // deno-lint-ignore no-explicit-any
    var core: any; // eslint-disable-line no-var
  }
}

unitTest(function bufferOpsHeaderTooShort(): void {
  for (let op in ["op_read_sync", "op_read_async"]) {
    const readOpId = Deno.core.ops()[op];
    const res = Deno.core.send(
      readOpId,
      new Uint8Array([
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        11,
      ]),
    );

    const headerByteLength = 4 * 4;
    assert(res.byteLength > headerByteLength);
    const view = new DataView(
      res.buffer,
      res.byteOffset + res.byteLength - headerByteLength,
      headerByteLength,
    );

    const requestId = Number(view.getBigUint64(0, true));
    const status = view.getUint32(8, true);
    const result = view.getUint32(12, true);

    assert(requestId === 0);
    assert(status !== 0);
    assertEquals(new TextDecoder().decode(res.slice(0, result)), "TypeError");
    assertEquals(
      new TextDecoder().decode(res.slice(result, -headerByteLength)).trim(),
      "Unparsable control buffer",
    );
  }
});
