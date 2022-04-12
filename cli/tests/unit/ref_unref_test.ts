import { assertNotEquals, execCode } from "./test_util.ts";

Deno.test("[unrefOp] unref'ing invalid ops does not have effects", async () => {
  const [statusCode, _] = await execCode(`
    Deno.core.unrefOp(-1);
    setTimeout(() => { throw new Error() }, 10)
  `);
  // Invalid unrefOp call doesn't affect exit condition of event loop
  assertNotEquals(statusCode, 0);
});
