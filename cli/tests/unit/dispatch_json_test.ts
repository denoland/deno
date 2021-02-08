import { assertMatch, assertStrictEquals, unitTest } from "./test_util.ts";

declare global {
  // deno-lint-ignore no-namespace
  namespace Deno {
    // deno-lint-ignore no-explicit-any
    var core: any; // eslint-disable-line no-var
  }
}

unitTest(function malformedJsonControlBuffer(): void {
  const opId = Deno.core.ops()["op_open_sync"];
  const argsBuf = new Uint8Array([1, 2, 3, 4, 5]);
  const resBuf = Deno.core.send(opId, argsBuf);
  const resText = new TextDecoder().decode(resBuf);
  const resObj = JSON.parse(resText);
  assertStrictEquals(resObj.ok, undefined);
  assertStrictEquals(resObj.err.className, "SyntaxError");
  assertMatch(resObj.err.message, /\bexpected value\b/);
});

unitTest(function invalidPromiseId(): void {
  const opId = Deno.core.ops()["op_open_async"];
  const promiseId = "1. NEIN!";
  const argsObj = {
    path: "/tmp/P.I.S.C.I.X/yeah",
    mode: 0o666,
    options: {
      read: true,
      write: true,
      create: true,
      truncate: false,
      append: false,
      createNew: false,
    },
  };
  const argsText = JSON.stringify(argsObj);
  const reqText = JSON.stringify([promiseId, argsText]);
  const reqBuf = new TextEncoder().encode(reqText);
  const resBuf = Deno.core.send(opId, reqBuf);
  const resText = new TextDecoder().decode(resBuf);
  const resObj = JSON.parse(resText);
  console.error(resText);
  assertStrictEquals(resObj.ok, undefined);
  assertStrictEquals(resObj.err.className, "TypeError");
  assertMatch(resObj.err.message, /\bpromiseId\b/);
});
