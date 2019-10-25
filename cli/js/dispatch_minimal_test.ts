import {
  test,
  assert,
  assertEquals,
  assertMatch,
  unreachable
} from "./test_util.ts";

const readErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(.*dispatch_minimal\\.ts:.*\\)
    at Object.sendAsync \\(.*dispatch_minimal\\.ts:.*\\)
    at async Object\\.open \\(.*files\\.ts:.*\\).*$`,
  "ms"
);

test(async function sendAsyncStackTrace(): Promise<void> {
  const buf = new Uint8Array(10);
  await Deno.read(10, "nonexistent.txt", buf)
    .then(unreachable)
    .catch(
      (error): void => {
        assertMatch(error.stack, readErrorStackPattern);
      }
    );
});
test(async function malformedMinimalControlBuffer(): Promise<void> {
  // @ts-ignore
  const res = Deno.core.send(1, new Uint8Array([1, 2, 3, 4, 5]));
  const header = res.slice(0, 12);
  const buf32 = new Int32Array(
    header.buffer,
    header.byteOffset,
    header.byteLength / 4
  );
  const arg = buf32[1];
  const result = buf32[2];
  const message = new TextDecoder().decode(res.slice(12));
  assert(arg < 0);
  assertEquals(result, Deno.ErrorKind.InvalidInput);
  assertEquals(message, "Unparsable control buffer");
});
