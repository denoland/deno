import { testPerm, assertMatch, unreachable } from "./test_util.ts";

const openErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(.*dispatch_json\\.ts:.*\\)
    at Object.sendAsync \\(.*dispatch_json\\.ts:.*\\)
    at async Object\\.open \\(.*files\\.ts:.*\\).*$`,
  "ms"
);

testPerm({ read: true }, async function sendAsyncStackTrace(): Promise<void> {
  await Deno.open("nonexistent.txt")
    .then(unreachable)
    .catch(
      (error): void => {
        assertMatch(error.stack, openErrorStackPattern);
      }
    );
});
