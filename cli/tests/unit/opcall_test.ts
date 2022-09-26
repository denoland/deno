import { assert, assertStringIncludes, unreachable } from "./test_util.ts";

Deno.test(async function sendAsyncStackTrace() {
  const buf = new Uint8Array(10);
  const rid = 10;
  try {
    await Deno.read(rid, buf);
    unreachable();
  } catch (error) {
    assert(error instanceof Error);
    const s = error.stack?.toString();
    assert(s);
    console.log(s);
    assertStringIncludes(s, "opcall_test.ts");
    assertStringIncludes(s, "read");
    assert(
      !s.includes("deno:core"),
      "opcall stack traces should NOT include deno:core internals such as unwrapOpResult",
    );
  }
});

declare global {
  namespace Deno {
    // deno-lint-ignore no-explicit-any, no-var
    var core: any;
  }
}

Deno.test(async function opsAsyncBadResource() {
  try {
    const nonExistingRid = 9999;
    await Deno.core.read(
      nonExistingRid,
      new Uint8Array(0),
    );
  } catch (e) {
    if (!(e instanceof Deno.errors.BadResource)) {
      throw e;
    }
  }
});

Deno.test(function opsSyncBadResource() {
  try {
    const nonExistingRid = 9999;
    Deno.core.ops.op_read_sync(nonExistingRid, new Uint8Array(0));
  } catch (e) {
    if (!(e instanceof Deno.errors.BadResource)) {
      throw e;
    }
  }
});
