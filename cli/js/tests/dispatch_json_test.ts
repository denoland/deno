import { assert, unitTest, assertMatch, unreachable } from "./test_util.ts";

const openErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(.*dispatch_json\\.ts:.*\\)
    at Object.sendAsync \\(.*dispatch_json\\.ts:.*\\)
    at async Object\\.open \\(.*files\\.ts:.*\\).*$`,
  "ms"
);

unitTest(
  { perms: { read: true } },
  async function sendAsyncStackTrace(): Promise<void> {
    await Deno.open("nonexistent.txt")
      .then(unreachable)
      .catch((error): void => {
        assertMatch(error.stack, openErrorStackPattern);
      });
  }
);

unitTest(function malformedJsonControlBuffer(): void {
  // @ts-ignore
  const opId = Deno.core.ops()["op_open"];
  // @ts-ignore
  const res = Deno.core.send(opId, new Uint8Array([1, 2, 3, 4, 5]));
  const resText = new TextDecoder().decode(res);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const resJson = JSON.parse(resText) as any;
  assert(!resJson.ok);
  assert(resJson.err);
});
