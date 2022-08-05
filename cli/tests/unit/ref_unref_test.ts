import { assertNotEquals, execCode } from "./test_util.ts";

Deno.test("[ops.op_unref_op] unref'ing invalid ops does not have effects", async () => {
  const [statusCode, _] = await execCode(`
    core.unwrapOpResult(Deno.core.ops.op_unref_op(-1));
    setTimeout(() => { throw new Error() }, 10)
  `);
  // Invalid ops.op_unref_op call doesn't affect exit condition of event loop
  assertNotEquals(statusCode, 0);
});
