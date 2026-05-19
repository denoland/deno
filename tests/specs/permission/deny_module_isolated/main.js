import { describeEnv, tryReadSelf } from "./helper.js";

const env = describeEnv();
const readAttempt = tryReadSelf();
console.log(
  JSON.stringify({
    helper_sees_globalThis_Deno: env.hasDeno,
    helper_sees_lexical_Deno: env.denoIsObject,
    helper_op_call_result: readAttempt,
  }),
);
