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
function RegisteredError(message) {
  registeredConstructorCalls++;
  const error = new Error(message);
  Object.setPrototypeOf(error, RegisteredError.prototype);
  return error;
}
RegisteredError.prototype = Object.create(Error.prototype);
const registeredPrototype = RegisteredError.prototype;
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

function callRegisteredError(buffer) {
  return ops.op_registered_err(buffer);
}

const fastBuffer = new Uint8Array(1);
// Keep one call site hot enough for V8 to dispatch through the generated fast
// callback, then make that same callback return an error.
for (let i = 0; i < 6000; i++) {
  callRegisteredError(fastBuffer);
}
fastBuffer[0] = 1;
let err;
try {
  callRegisteredError(fastBuffer);
  throw new Error("op_registered_err didn't throw!");
} catch (error) {
  err = error;
}
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
if (fastBuffer.byteLength !== 1) {
  throw new Error("fast op buffer was detached");
}

// Replacing a writable function prototype after registration must not change
// the prototype captured by native error construction.
RegisteredError.prototype = Object.create(Error.prototype);
try {
  callRegisteredError(fastBuffer);
  throw new Error("op_registered_err didn't throw!");
} catch (error) {
  err = error;
}
if (Object.getPrototypeOf(err) !== registeredPrototype) {
  throw new Error("registered prototype was not snapshotted");
}
