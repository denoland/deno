// Copyright 2018-2026 the Deno authors. MIT license.
// deno-fmt-ignore-file

(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  Error,
  PromisePrototypeThen,
  ArrayPrototypePop,
  NumberIsInteger,
  ObjectGetOwnPropertyNames,
  ReflectGetOwnPropertyDescriptor,
  ObjectDefineProperty,
  NumberIsSafeInteger,
  FunctionPrototypeApply,
  SafeArrayIterator,
} = primordials;
const { TextDecoder, TextEncoder } = core.loadExtScript(
  "ext:deno_web/08_text_encoding.js",
);
const { errorMap } = core.loadExtScript("ext:deno_node/internal_binding/uv.ts");
const { codes } = core.loadExtScript("ext:deno_node/internal/error_codes.ts");
const { ERR_NOT_IMPLEMENTED } = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { validateNumber } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);

type BinaryEncodings = "binary";

type TextEncodings =
  | "ascii"
  | "utf8"
  | "utf-8"
  | "utf16le"
  | "ucs2"
  | "ucs-2"
  | "base64"
  | "base64url"
  | "latin1"
  | "hex";

type Encodings = BinaryEncodings | TextEncodings;

function notImplemented(msg: string): never {
  throw new ERR_NOT_IMPLEMENTED(msg);
}

function warnNotImplemented(msg?: string) {
  const message = msg
    ? `Warning: Not implemented: ${msg}`
    : "Warning: Not implemented";
  // deno-lint-ignore no-console
  console.warn(message);
}

type _TextDecoder = typeof TextDecoder.prototype;
const _TextDecoder = TextDecoder;

type _TextEncoder = typeof TextEncoder.prototype;
const _TextEncoder = TextEncoder;

// API helpers

type MaybeNull<T> = T | null;
type MaybeDefined<T> = T | undefined;
type MaybeEmpty<T> = T | null | undefined;

function intoCallbackAPI<T>(
  // deno-lint-ignore no-explicit-any
  func: (...args: any[]) => Promise<T>,
  cb: MaybeEmpty<(err: MaybeNull<Error>, value?: MaybeEmpty<T>) => void>,
  // deno-lint-ignore no-explicit-any
  ...args: any[]
) {
  PromisePrototypeThen(
    func(...new SafeArrayIterator(args)),
    (value: T) => cb && cb(null, value),
    (err: MaybeNull<Error>) => cb && cb(err),
  );
}

function intoCallbackAPIWithIntercept<T1, T2>(
  // deno-lint-ignore no-explicit-any
  func: (...args: any[]) => Promise<T1>,
  interceptor: (v: T1) => T2,
  cb: MaybeEmpty<(err: MaybeNull<Error>, value?: MaybeEmpty<T2>) => void>,
  // deno-lint-ignore no-explicit-any
  ...args: any[]
) {
  PromisePrototypeThen(
    func(...new SafeArrayIterator(args)),
    (value: T1) => cb && cb(null, interceptor(value)),
    (err: MaybeNull<Error>) => cb && cb(err),
  );
}

function spliceOne(list: string[], index: number) {
  for (; index + 1 < list.length; index++) {
    list[index] = list[index + 1];
  }
  ArrayPrototypePop(list);
}

function validateIntegerRange(
  value: number,
  name: string,
  min = -2147483648,
  max = 2147483647,
) {
  // The defaults for min and max correspond to the limits of 32-bit integers.
  if (!NumberIsInteger(value)) {
    throw new Error(`${name} must be 'an integer' but was ${value}`);
  }

  if (value < min || value > max) {
    throw new Error(
      `${name} must be >= ${min} && <= ${max}. Value was ${value}`,
    );
  }
}

type OptionalSpread<T> = T extends undefined ? []
  : [T];

function once<T = undefined>(
  callback: (...args: OptionalSpread<T>) => void,
) {
  let called = false;
  return function (this: unknown, ...args: OptionalSpread<T>) {
    if (called) return;
    called = true;
    FunctionPrototypeApply(callback, this, args);
  };
}

function makeMethodsEnumerable(klass: { new (): unknown }) {
  const proto = klass.prototype;
  const names = ObjectGetOwnPropertyNames(proto);
  for (let i = 0; i < names.length; i++) {
    const key = names[i];
    const value = proto[key];
    if (typeof value === "function") {
      const desc = ReflectGetOwnPropertyDescriptor(proto, key);
      if (desc) {
        desc.enumerable = true;
        ObjectDefineProperty(proto, key, desc);
      }
    }
  }
}

/**
 * Returns a system error name from an error code number.
 * @param code error code number
 */
function getSystemErrorName(code: number): string | undefined {
  validateNumber(code, "err");
  if (code >= 0 || !NumberIsSafeInteger(code)) {
    throw new codes.ERR_OUT_OF_RANGE("err", "a negative integer", code);
  }
  return errorMap.get(code)?.[0];
}

/**
 * Returns a system error message from an error code number.
 * @param code error code number
 */
function getSystemErrorMessage(code: number): string | undefined {
  validateNumber(code, "err");
  if (code >= 0 || !NumberIsSafeInteger(code)) {
    throw new codes.ERR_OUT_OF_RANGE("err", "a negative integer", code);
  }
  return errorMap.get(code)?.[1];
}

return {
  notImplemented,
  warnNotImplemented,
  _TextDecoder,
  _TextEncoder,
  intoCallbackAPI,
  intoCallbackAPIWithIntercept,
  spliceOne,
  validateIntegerRange,
  once,
  makeMethodsEnumerable,
  getSystemErrorName,
  getSystemErrorMessage,
};
})()
