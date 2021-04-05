import { assert, assertEquals, assertMatch, unreachable } from "./test_util.ts";

const readErrorStackPattern = new RegExp(
  `^.*
    at processErr \\(.*core\\.js:.*\\)
    at opAsyncHandler \\(.*core\\.js:.*\\)
    at handleAsyncMsgFromRust \\(.*core\\.js:.*\\).*$`,
  "ms",
);

Deno.test("sendAsyncStackTrace", async function (): Promise<void> {
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
