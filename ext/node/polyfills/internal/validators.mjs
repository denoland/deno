// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ArrayIsArray,
  ArrayPrototypeIncludes,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  NumberIsInteger,
  NumberIsNaN,
  NumberMIN_SAFE_INTEGER,
  NumberMAX_SAFE_INTEGER,
  NumberParseInt,
  NumberIsFinite,
  SafeRegExp,
  String,
  StringPrototypeTrim,
  ReflectHas,
  RegExpPrototypeTest,
} = primordials;

import { codes } from "ext:deno_node/internal/error_codes.ts";
import { hideStackFrames } from "ext:deno_node/internal/hide_stack_frames.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { normalizeEncoding } from "ext:deno_node/internal/util.mjs";

/**
 * @param {number} value
 * @returns {boolean}
 */
function isInt32(value) {
  return value === (value | 0);
}

/**
 * @param {unknown} value
 * @returns {boolean}
 */
function isUint32(value) {
  return value === (value >>> 0);
}

const octalReg = new SafeRegExp(/^[0-7]+$/);
const modeDesc = "must be a 32-bit unsigned integer or an octal string";

/**
 * Parse and validate values that will be converted into mode_t (the S_*
 * constants). Only valid numbers and octal strings are allowed. They could be
 * converted to 32-bit unsigned integers or non-negative signed integers in the
 * C++ land, but any value higher than 0o777 will result in platform-specific
 * behaviors.
 *
 * @param {*} value Values to be validated
 * @param {string} name Name of the argument
 * @param {number} [def] If specified, will be returned for invalid values
 * @returns {number}
 */
function parseFileMode(value, name, def) {
  value ??= def;
  if (typeof value === "string") {
    if (!RegExpPrototypeTest(octalReg, value)) {
      throw new codes.ERR_INVALID_ARG_VALUE(name, value, modeDesc);
    }
    value = NumberParseInt(value, 8);
  }

  validateInt32(value, name, 0, 2 ** 32 - 1);
  return value;
}

/** @type {(buffer: unknown, name?: string) => asserts buffer is ArrayBufferView} */
const validateBuffer = hideStackFrames((buffer, name = "buffer") => {
  if (!isArrayBufferView(buffer)) {
    throw new codes.ERR_INVALID_ARG_TYPE(
      name,
      ["Buffer", "TypedArray", "DataView"],
      buffer,
    );
  }
});

/** @type {ValidateNumber} */
const validateInteger = hideStackFrames(
  (
    value,
    name,
    min = NumberMIN_SAFE_INTEGER,
    max = NumberMAX_SAFE_INTEGER,
  ) => {
    if (typeof value !== "number") {
      throw new codes.ERR_INVALID_ARG_TYPE(name, "number", value);
    }
    if (!NumberIsInteger(value)) {
      throw new codes.ERR_OUT_OF_RANGE(name, "an integer", value);
    }
    if (value < min || value > max) {
      throw new codes.ERR_OUT_OF_RANGE(name, `>= ${min} && <= ${max}`, value);
    }
  },
);

/**
 * @typedef {{
 *   allowArray?: boolean,
 *   allowFunction?: boolean,
 *   nullable?: boolean,
 * }} ValidateObjectOptions
 */
/** @typedef {(value: unknown, name: string, options?: ValidateObjectOptions) => asserts value is object} ValidateObject */
/** @type {ValidateObject} */
const validateObject = hideStackFrames((value, name, options) => {
  const useDefaultOptions = options == null;
  const allowArray = useDefaultOptions ? false : options.allowArray;
  const allowFunction = useDefaultOptions ? false : options.allowFunction;
  const nullable = useDefaultOptions ? false : options.nullable;
  if (
    (!nullable && value === null) ||
    (!allowArray && ArrayIsArray(value)) ||
    (typeof value !== "object" && (
      !allowFunction || typeof value !== "function"
    ))
  ) {
    throw new codes.ERR_INVALID_ARG_TYPE(name, "Object", value);
  }
});

/** @type {ValidateNumber} */
const validateInt32 = hideStackFrames(
  (value, name, min = -2147483648, max = 2147483647) => {
    // The defaults for min and max correspond to the limits of 32-bit integers.
    if (!isInt32(value)) {
      if (typeof value !== "number") {
        throw new codes.ERR_INVALID_ARG_TYPE(name, "number", value);
      }

      if (!NumberIsInteger(value)) {
        throw new codes.ERR_OUT_OF_RANGE(name, "an integer", value);
      }

      throw new codes.ERR_OUT_OF_RANGE(name, `>= ${min} && <= ${max}`, value);
    }

    if (value < min || value > max) {
      throw new codes.ERR_OUT_OF_RANGE(name, `>= ${min} && <= ${max}`, value);
    }
  },
);

/**
 * @type {(value: unknown, name: string, positive?: boolean) => asserts value is number}
 */
const validateUint32 = hideStackFrames(
  (value, name, positive) => {
    if (!isUint32(value)) {
      if (typeof value !== "number") {
        throw new codes.ERR_INVALID_ARG_TYPE(name, "number", value);
      }
      if (!NumberIsInteger(value)) {
        throw new codes.ERR_OUT_OF_RANGE(name, "an integer", value);
      }
      const min = positive ? 1 : 0;
      // 2 ** 32 === 4294967296
      throw new codes.ERR_OUT_OF_RANGE(
        name,
        `>= ${min} && < 4294967296`,
        value,
      );
    }
    if (positive && value === 0) {
      throw new codes.ERR_OUT_OF_RANGE(name, ">= 1 && < 4294967296", value);
    }
  },
);

/**
 * @param {unknown} value
 * @param {string} name
 */
function validateString(value, name) {
  if (typeof value !== "string") {
    throw new codes.ERR_INVALID_ARG_TYPE(name, "string", value);
  }
}

/** @typedef {(value: unknown, name: string, min?: number, max?: number) => asserts value is number} ValidateNumber */
/** @type {ValidateNumber} */
const validateNumber = hideStackFrames((value, name, min = undefined, max) => {
  if (typeof value !== "number") {
    throw new codes.ERR_INVALID_ARG_TYPE(name, "number", value);
  }

  if (
    (min != null && value < min) || (max != null && value > max) ||
    ((min != null || max != null) && NumberIsNaN(value))
  ) {
    throw new codes.ERR_OUT_OF_RANGE(
      name,
      `${min != null ? `>= ${min}` : ""}${
        min != null && max != null ? " && " : ""
      }${max != null ? `<= ${max}` : ""}`,
      value,
    );
  }
});

/**
 * @param {unknown} value
 * @param {string} name
 */
function validateBoolean(value, name) {
  if (typeof value !== "boolean") {
    throw new codes.ERR_INVALID_ARG_TYPE(name, "boolean", value);
  }
}

/** @typedef {<T>(value: unknown, name: string, oneOf: readonly T[]) => asserts value is T} ValidateOneOf */
/** @type {ValidateOneOf} */
const validateOneOf = hideStackFrames((value, name, oneOf) => {
  if (!ArrayPrototypeIncludes(oneOf, value)) {
    const allowed = ArrayPrototypeJoin(
      ArrayPrototypeMap(
        oneOf,
        (v) => (typeof v === "string" ? `'${v}'` : String(v)),
      ),
      ", ",
    );
    const reason = "must be one of: " + allowed;

    throw new codes.ERR_INVALID_ARG_VALUE(name, value, reason);
  }
});

export function validateEncoding(data, encoding) {
  const normalizedEncoding = normalizeEncoding(encoding);
  const length = data.length;

  if (normalizedEncoding === "hex" && length % 2 !== 0) {
    throw new codes.ERR_INVALID_ARG_VALUE(
      "encoding",
      encoding,
      `is invalid for data of length ${length}`,
    );
  }
}

// Check that the port number is not NaN when coerced to a number,
// is an integer and that it falls within the legal range of port numbers.
/**
 * @param {string} name
 * @returns {number}
 */
function validatePort(port, name = "Port", allowZero = true) {
  if (
    (typeof port !== "number" && typeof port !== "string") ||
    (typeof port === "string" &&
      StringPrototypeTrim(port).length === 0) ||
    +port !== (+port >>> 0) ||
    port > 0xFFFF ||
    (port === 0 && !allowZero)
  ) {
    throw new codes.ERR_SOCKET_BAD_PORT(name, port, allowZero);
  }

  return port;
}

/**
 * @param {unknown} signal
 * @param {string} name
 */
const validateAbortSignal = hideStackFrames(
  (signal, name) => {
    if (
      signal !== undefined &&
      (signal === null ||
        typeof signal !== "object" ||
        !ReflectHas(signal, "aborted"))
    ) {
      throw new codes.ERR_INVALID_ARG_TYPE(name, "AbortSignal", signal);
    }
  },
);

/**
 * @param {unknown} value
 * @param {string} name
 */
const validateFunction = hideStackFrames(
  (value, name) => {
    if (typeof value !== "function") {
      throw new codes.ERR_INVALID_ARG_TYPE(name, "Function", value);
    }
  },
);

/**
 * @param {unknown} value
 * @param {string} name
 */
const validateArray = hideStackFrames(
  (value, name, minLength = 0) => {
    if (!ArrayIsArray(value)) {
      throw new codes.ERR_INVALID_ARG_TYPE(name, "Array", value);
    }
    if (value.length < minLength) {
      const reason = `must be longer than ${minLength}`;
      throw new codes.ERR_INVALID_ARG_VALUE(name, value, reason);
    }
  },
);

/**
 * @callback validateStringArray
 * @param {*} value
 * @param {string} name
 * @returns {asserts value is string[]}
 */

/** @type {validateStringArray} */
const validateStringArray = hideStackFrames((value, name) => {
  validateArray(value, name);
  for (let i = 0; i < value.length; ++i) {
    // Don't use validateString here for performance reasons, as
    // we would generate intermediate strings for the name.
    if (typeof value[i] !== "string") {
      throw new codes.ERR_INVALID_ARG_TYPE(`${name}[${i}]`, "string", value[i]);
    }
  }
});

/**
 * @callback validateBooleanArray
 * @param {*} value
 * @param {string} name
 * @returns {asserts value is boolean[]}
 */

/** @type {validateBooleanArray} */
const validateBooleanArray = hideStackFrames((value, name) => {
  validateArray(value, name);
  for (let i = 0; i < value.length; ++i) {
    // Don't use validateBoolean here for performance reasons, as
    // we would generate intermediate strings for the name.
    if (value[i] !== true && value[i] !== false) {
      throw new codes.ERR_INVALID_ARG_TYPE(
        `${name}[${i}]`,
        "boolean",
        value[i],
      );
    }
  }
});

function validateUnion(value, name, union) {
  if (!ArrayPrototypeIncludes(union, value)) {
    throw new codes.ERR_INVALID_ARG_TYPE(
      name,
      `('${ArrayPrototypeJoin(union, "|")}')`,
      value,
    );
  }
}

const validateFiniteNumber = hideStackFrames((number, name) => {
  // Common case
  if (number === undefined) {
    return false;
  }

  if (NumberIsFinite(number)) {
    return true; // Is a valid number
  }

  if (NumberIsNaN(number)) {
    return false;
  }

  validateNumber(number, name);

  // Infinite numbers
  throw new codes.ERR_OUT_OF_RANGE(name, "a finite number", number);
});

const checkRangesOrGetDefault = hideStackFrames(
  (number, name, lower, upper, def) => {
    if (!validateFiniteNumber(number, name)) {
      return def;
    }
    if (number < lower || number > upper) {
      throw new codes.ERR_OUT_OF_RANGE(
        name,
        `>= ${lower} and <= ${upper}`,
        number,
      );
    }
    return number;
  },
);

export default {
  isInt32,
  isUint32,
  parseFileMode,
  validateAbortSignal,
  validateArray,
  validateBoolean,
  validateBooleanArray,
  validateBuffer,
  validateFunction,
  validateInt32,
  validateInteger,
  validateNumber,
  validateObject,
  validateOneOf,
  validatePort,
  validateString,
  validateStringArray,
  validateUint32,
  validateUnion,
  validateFiniteNumber,
  checkRangesOrGetDefault,
};
export {
  checkRangesOrGetDefault,
  isInt32,
  isUint32,
  parseFileMode,
  validateAbortSignal,
  validateArray,
  validateBoolean,
  validateBooleanArray,
  validateBuffer,
  validateFiniteNumber,
  validateFunction,
  validateInt32,
  validateInteger,
  validateNumber,
  validateObject,
  validateOneOf,
  validatePort,
  validateString,
  validateStringArray,
  validateUint32,
  validateUnion,
};
