import { testPerm, assertMatch, unreachable } from "./test_util.ts";

const openErrorStackPattern = new RegExp(
  `^.*
    at unwrapResponse \\(js\\/dispatch_json\\.ts:.*\\)
    at sendAsync.* \\(js\\/dispatch_json\\.ts:.*\\)
    at async Object\\.open \\(js\\/files\\.ts:.*\\).*$`,
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
