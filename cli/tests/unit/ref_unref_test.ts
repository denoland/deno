import { assertThrows } from "./test_util.ts";

Deno.test("ref/unref throws when called with invalid promise ids", () => {
  assertThrows(
    () => {
      Deno.core.refOp(-1);
    },
    Error,
    "Async op of the given promise id doesn't exist",
  );
  assertThrows(
    () => {
      Deno.core.unrefOp(-1);
    },
    Error,
    "Async op of the given promise id doesn't exist",
  );
});

Deno.test("ref/unref doesn't throw when called with valid promise ids", () => {
  const cancelId = Deno.core.opSync("op_timer_handle");
  const op = Deno.core.opAsync("op_sleep", 100, cancelId);
  op.catch(() => {/* ignore error */});
  Deno.core.unrefOp(op[Symbol.for("Deno.core.internalPromiseId")]);
  Deno.core.refOp(op[Symbol.for("Deno.core.internalPromiseId")]);
  Deno.core.tryClose(cancelId);
});
