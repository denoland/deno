import { assertMatch, unitTest, unreachable } from "./test_util.ts";

// IMPORTANT: This pattern ensures that async op stack traces are grounded in
// user code and not the global async receiver. It may need to be adjusted for
// file/function renames, but preserve the intention.
const readErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(deno:core/core\\.js:.*\\)
    at jsonOpAsync \\(deno:core/core\\.js:.*\\)
    at async Object\\.read \\(deno:runtime/js/12_io\\.js:.*\\)
    at async .*/dispatch_bin_test\\.ts:.*$`,
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
