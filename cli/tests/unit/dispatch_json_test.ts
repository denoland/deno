import { assertMatch, assertStrictEquals } from "./test_util.ts";

declare global {
  // deno-lint-ignore no-namespace
  namespace Deno {
    // deno-lint-ignore no-explicit-any
    var core: any; // eslint-disable-line no-var
  }
}

Deno.test("malformedJsonControlBuffer", function (): void {
  const opId = Deno.core.ops()["op_open_sync"];
  const argsBuf = new Uint8Array([1, 2, 3, 4, 5]);
  const resBuf = Deno.core.send(opId, argsBuf);
  const resText = new TextDecoder().decode(resBuf);
  const resObj = JSON.parse(resText);
  assertStrictEquals(resObj.ok, undefined);
  assertStrictEquals(resObj.err.className, "SyntaxError");
  assertMatch(resObj.err.message, /\bexpected value\b/);
});

Deno.test("invalidPromiseId", function (): void {
  const opId = Deno.core.ops()["op_open_async"];
  const reqBuf = new Uint8Array([0, 0, 0, 0, 0, 0, 0]);
  const resBuf = Deno.core.send(opId, reqBuf);
  const resText = new TextDecoder().decode(resBuf);
  const resObj = JSON.parse(resText);
  console.error(resText);
  assertStrictEquals(resObj.ok, undefined);
  assertStrictEquals(resObj.err.className, "TypeError");
  assertMatch(resObj.err.message, /\brequestId\b/);
});
