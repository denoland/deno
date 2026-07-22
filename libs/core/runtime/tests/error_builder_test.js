// Copyright 2018-2026 the Deno authors. MIT license.
const { core } = Deno;
const { ops } = core;

const domExceptionBrand = Symbol("DOMException brand");
const domExceptionMessage = Symbol("DOMException message");
class DOMException {
  constructor(message, code) {
    this[domExceptionBrand] = true;
    this[domExceptionMessage] = message;
    this.code = code;
  }

  get msg() {
    if (this[domExceptionBrand] !== true) {
      throw new TypeError("Illegal invocation");
    }
    return this[domExceptionMessage];
  }
}

core.registerErrorBuilder(
  "DOMExceptionOperationError",
  function DOMExceptionOperationError(msg) {
    return new DOMException(msg, "OperationError");
  },
);

let registeredConstructorCalls = 0;
let registeredNameSetterCalls = 0;
class RegisteredError extends Error {
  constructor(message) {
    super(message);
    registeredConstructorCalls++;
  }
}
core.registerErrorClass("RegisteredError", RegisteredError);
const registeredDescriptor = Object.getOwnPropertyDescriptor(
  core.errorConstructors,
  "RegisteredError",
);
if (registeredDescriptor.writable || registeredDescriptor.configurable) {
  throw new Error("registered constructor entry is mutable");
}
let inheritedConstructorLookupCalls = 0;
Object.setPrototypeOf(core.errorConstructors, {
  get DOMExceptionOperationError() {
    inheritedConstructorLookupCalls++;
    return RegisteredError;
  },
});
let ownConstructorLookupCalls = 0;
Object.defineProperty(
  core.errorConstructors,
  "DOMExceptionOperationError",
  {
    configurable: true,
    get() {
      ownConstructorLookupCalls++;
      return RegisteredError;
    },
  },
);
Object.defineProperty(RegisteredError.prototype, "name", {
  configurable: true,
  set() {
    registeredNameSetterCalls++;
  },
});

function assertBuilderOnlyError() {
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
}

assertBuilderOnlyError();
if (ownConstructorLookupCalls !== 0) {
  throw new Error("own constructor accessor was consulted");
}
delete core.errorConstructors.DOMExceptionOperationError;
assertBuilderOnlyError();
if (inheritedConstructorLookupCalls !== 0) {
  throw new Error("inherited constructor lookup was consulted");
}

try {
  ops.op_registered_err(new Uint8Array());
  throw new Error("op_registered_err didn't throw!");
} catch (err) {
  if (!(err instanceof RegisteredError)) {
    throw new Error("err not RegisteredError");
  }
  if (registeredConstructorCalls !== 0) {
    throw new Error("registered constructor was called");
  }
  if (registeredNameSetterCalls !== 0) {
    throw new Error("registered name setter was called");
  }
  if (!Object.hasOwn(err, "name") || err.name !== "RegisteredError") {
    throw new Error("err.name is incorrect");
  }
  if (err.message !== "registered message") {
    throw new Error("err.message is incorrect");
  }
}
