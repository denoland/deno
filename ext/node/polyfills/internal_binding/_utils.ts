// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  op_base64url_decode,
} = core.ops;
const {
  forgivingBase64Decode,
} = core.loadExtScript("ext:deno_web/00_infra.js");
const {
  DataView,
  DataViewPrototypeSetUint16,
  Error,
  Int8Array,
  MathMin,
  NumberPOSITIVE_INFINITY,
  NumberPrototypeToString,
  SafeRegExp,
  StringPrototypeCharCodeAt,
  StringPrototypeIndexOf,
  StringPrototypeReplace,
  StringPrototypeReplaceAll,
  StringPrototypeSubstring,
  StringPrototypeTrim,
  StringPrototypeTrimStart,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeSubarray,
  Uint8Array,
} = primordials;

function asciiToBytes(str: string) {
  const length = str.length;
  const byteArray = new Uint8Array(length);
  for (let i = 0; i < length; ++i) {
    byteArray[i] = StringPrototypeCharCodeAt(str, i) & 255;
  }
  return byteArray;
}

function base64ToBytes(str: string) {
  try {
    return forgivingBase64Decode(str);
  } catch {
    return base64CleanToBytes(str);
  }
}

// Node's cleaning semantics for dirty base64 input. Callers that already
// know the input is invalid (a -1 op sentinel) come here directly, skipping
// base64ToBytes' first decode attempt, which would always throw.
function base64CleanToBytes(str: string) {
  // Convert base64url characters to standard base64 before cleaning,
  // so that the padding logic in base64clean works correctly.
  str = StringPrototypeReplaceAll(
    StringPrototypeReplaceAll(str, "-", "+"),
    "_",
    "/",
  );
  str = base64clean(str);
  return forgivingBase64Decode(str);
}

const INVALID_BASE64_RE = new SafeRegExp(/[^+/0-9A-Za-z-_]/g);
function base64clean(str: string) {
  // Node takes equal signs as end of the Base64 encoding
  const eqIndex = StringPrototypeIndexOf(str, "=");
  str = eqIndex !== -1
    ? StringPrototypeTrimStart(StringPrototypeSubstring(str, 0, eqIndex))
    : StringPrototypeTrim(str);
  // Node strips out invalid characters like \n and \t from the string, std/base64 does not
  str = StringPrototypeReplace(str, INVALID_BASE64_RE, "");
  // Node converts strings with length < 2 to ''
  const length = str.length;
  if (length < 2) return "";
  // Node allows for non-padded base64 strings (missing trailing ===), std/base64 does not
  switch (length % 4) {
    case 0:
      return str;
    case 1:
      // A single base64 char can't encode a full byte; drop it like Node does
      return StringPrototypeSubstring(str, 0, length - 1);
    case 2:
      return `${str}==`;
    case 3:
      return `${str}=`;
    default:
      throw new Error("Unexpected NaN value for string length");
  }
}

function base64UrlToBytes(str: string) {
  str = base64clean(str);
  // Node's base64url decode also accepts the standard alphabet; the Url op
  // does not, so this conversion is load-bearing.
  str = StringPrototypeReplaceAll(
    StringPrototypeReplaceAll(str, "+", "-"),
    "/",
    "_",
  );
  return op_base64url_decode(str);
}

// https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/deps/nbytes/src/nbytes.cpp#L144-L158
// deno-fmt-ignore
const unhexTable = new Int8Array([
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 0 - 15
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 16 - 31
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 32 - 47
     0,  1,  2,  3,  4,  5,  6,  7,  8,  9, -1, -1, -1, -1, -1, -1, // 48 - 63
    -1, 10, 11, 12, 13, 14, 15, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 64 - 79
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 80 - 95
    -1, 10, 11, 12, 13, 14, 15, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 96 - 111
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 112 - 127
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 128 ...
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // ... 255
  ]);

function hexToBytes(str: string) {
  const length = str.length >>> 1;
  const byteArray = new Uint8Array(length);
  let i: number;
  for (i = 0; i < length; i++) {
    const a = unhexTable[StringPrototypeCharCodeAt(str, i * 2) & 0xff];
    const b = unhexTable[StringPrototypeCharCodeAt(str, i * 2 + 1) & 0xff];
    if (!~a || !~b) {
      break;
    }
    byteArray[i] = (a << 4) | b;
  }
  // Returning a buffer subarray is okay: This API's return value
  // is never exposed to users and is only ever used for its length
  // and the data within the subarray.
  return i === length
    ? byteArray
    : TypedArrayPrototypeSubarray(byteArray, 0, i);
}

// JS fallback for when native Uint8Array.prototype.toHex is unavailable.
function bytesToHex(bytes: Uint8Array) {
  const length = TypedArrayPrototypeGetLength(bytes);
  let out = "";
  for (let i = 0; i < length; i++) {
    const b = bytes[i];
    out += (b < 0x10 ? "0" : "") + NumberPrototypeToString(b, 16);
  }
  return out;
}

function utf16leToBytes(str: string, units?: number) {
  // If units is defined, round it to even values for 16 byte "steps"
  // and use it as an upper bound value for our string byte array's length.
  const length = MathMin(
    str.length * 2,
    units ? (units >>> 1) * 2 : NumberPOSITIVE_INFINITY,
  );
  const byteArray = new Uint8Array(length);
  const view = new DataView(TypedArrayPrototypeGetBuffer(byteArray));
  let i: number;
  for (i = 0; i * 2 < length; i++) {
    DataViewPrototypeSetUint16(
      view,
      i * 2,
      StringPrototypeCharCodeAt(str, i),
      true,
    );
  }
  // Returning a buffer subarray is okay: This API's return value
  // is never exposed to users and is only ever used for its length
  // and the data within the subarray.
  return i * 2 === length
    ? byteArray
    : TypedArrayPrototypeSubarray(byteArray, 0, i * 2);
}

return {
  asciiToBytes,
  base64CleanToBytes,
  base64ToBytes,
  base64UrlToBytes,
  bytesToHex,
  hexToBytes,
  utf16leToBytes,
  unhexTable,
};
})();
