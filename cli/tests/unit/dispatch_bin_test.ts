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

unitTest(async function binOpsAsyncBadResource(): Promise<void> {
  try {
    const nonExistingRid = 9999;
    await Deno.core.binOpAsync(
      "op_read_async",
      nonExistingRid,
      new Uint8Array(0),
    );
  } catch (e) {
    if (!(e instanceof Deno.errors.BadResource)) {
      throw e;
    }
  }
});

unitTest(function binOpsSyncBadResource(): void {
  try {
    const nonExistingRid = 9999;
    Deno.core.binOpSync("op_read_sync", nonExistingRid, new Uint8Array(0));
  } catch (e) {
    if (!(e instanceof Deno.errors.BadResource)) {
      throw e;
    }
  }
});
