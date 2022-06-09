import { assertThrows } from "../../../test_util/std/testing/asserts.ts";

assertThrows(
  () => Deno.core.opSync("op_set_exit_code", 42),
  Error,
  "Deno.exit() is not supported in worker contexts",
);

assertThrows(
  () => Deno.exit(),
  Error,
  "Deno.exit() is not supported in worker contexts",
);

self.postMessage("ok");
