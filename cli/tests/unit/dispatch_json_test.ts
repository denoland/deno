import {
  assertStrictEquals,
  unitTest,
  assertMatch,
  unreachable,
} from "./test_util.ts";

const openErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(.*dispatch_json\\.js:.*\\)
    at sendAsync \\(.*dispatch_json\\.js:.*\\)
    at async Object\\.open \\(.*files\\.js:.*\\).*$`,
  "ms",
);

unitTest(
  { perms: { read: true } },
  async function sendAsyncStackTrace(): Promise<void> {
    await Deno.open("nonexistent.txt")
      .then(unreachable)
      .catch((error): void => {
        assertMatch(error.stack, openErrorStackPattern);
      });
  },
);

declare global {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace Deno {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
  assertStrictEquals(resObj.err.kind, "TypeError");
  assertMatch(resObj.err.message, /\bexpected value\b/);
});

unitTest(function invalidPromiseId(): void {
  const opId = Deno.core.ops()["op_open_async"];
  const argsObj = {
    promiseId: "1. NEIN!",
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
  const argsBuf = new TextEncoder().encode(argsText);
  const resBuf = Deno.core.send(opId, argsBuf);
  const resText = new TextDecoder().decode(resBuf);
  const resObj = JSON.parse(resText);
  console.error(resText);
  assertStrictEquals(resObj.ok, undefined);
  assertStrictEquals(resObj.err.kind, "TypeError");
  assertMatch(resObj.err.message, /\bpromiseId\b/);
});
