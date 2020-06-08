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

/* eslint-disable @typescript-eslint/no-namespace, @typescript-eslint/no-explicit-any,no-var */
declare global {
  namespace Deno {
    var core: any;
  }
}
/* eslint-enable */

unitTest(function malformedJsonControlBuffer(): void {
  const opId = Deno.core.ops()["op_open"];
  const res = Deno.core.send(opId, new Uint8Array([1, 2, 3, 4, 5]));
  const resText = new TextDecoder().decode(res);
  const resJson = JSON.parse(resText);
  assert(!resJson.ok);
  assert(resJson.err);
});
