// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const { core } = Deno;

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
  core.opSync("op_err", undefined, null);
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
