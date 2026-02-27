// Copyright 2018-2025 the Deno authors. MIT license.
const { core } = Deno;
const { ops } = core;

class DOMException {
  constructor(message, code) {
    this.msg = message;
    this.code = code;
  }
}

core.registerErrorBuilder(
  "DOMExceptionOperationError",
  function DOMExceptionOperationError(msg) {
    return new DOMException(msg, "OperationError");
  },
);

try {
  ops.op_err();
  throw new Error("op_err didn't throw!");
} catch (err) {
  if (!(err instanceof DOMException)) {
    throw new Error("err not DOMException");
  }
  if (err.msg !== "abc") {
    throw new Error("err.message is incorrect");
  }
  if (err.code !== "OperationError") {
    throw new Error("err.code is incorrect");
  }
}
