// Ported from punycode v2.1.1
// https://github.com/bestiejs/punycode.js/tree/v2.1.1
// Copyright 2011-2018 by Mathias Bynens. All rights reserved. MIT licence.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** Highest positive signed 32-bit float value */
const maxInt = 2147483647; // aka. 0x7FFFFFFF or 2^31-1

/** Bootstring parameters */
const base = 36;
const tMin = 1;
const tMax = 26;
const skew = 38;
const damp = 700;
const initialBias = 72;
const initialN = 128; // 0x80
const delimiter = "-"; // '\x2D'

/** Regular expressions */
const regexPunycode = /^xn--/;
const regexNonASCII = /[^\0-\x7E]/; // non-ASCII chars
const regexSeparators = /[\x2E\u3002\uFF0E\uFF61]/g; // RFC 3490 separators

/** Error messages */
const errors = {
  overflow: "Overflow: input needs wider integers to process",
  "not-basic": "Illegal input >= 0x80 (not a basic code point)",
  "invalid-input": "Invalid input",
};

/** Convenience shortcuts */
const baseMinusTMin = base - tMin;
const floor = Math.floor;
const stringFromCharCode = String.fromCharCode;

/*--------------------------------------------------------------------------*/

/** A generic error utility function. */
function error(type: keyof typeof errors): never {
  throw new RangeError(errors[type]);
}

/** A generic `Array#map` utility function. */
function map<T, U>(array: T[], fn: (el: T) => U): U[] {
  const result = [];
  let length = array.length;
  while (length--) {
    result[length] = fn(array[length]);
  }
  return result;
}

/** A simple `Array#map`-like wrapper to work with domain name strings or email
 * addresses. */
function mapDomain(string: string, fn: (c: string) => string): string {
  const parts = string.split("@");
  let result = "";
  if (parts.length > 1) {
    // In email addresses, only the domain name should be punycoded. Leave
    // the local part (i.e. everything up to `@`) intact.
    result = parts[0] + "@";
    string = parts[1];
  }
  // Avoid `split(regex)` for IE8 compatibility. See #17.
  string = string.replace(regexSeparators, "\x2E");
  const labels = string.split(".");
  const encoded = map(labels, fn).join(".");
  return result + encoded;
}

/** Creates an array containing the numeric code points of each Unicode
 * character in the string. While JavaScript uses UCS-2 internally,
 * this function will convert a pair of surrogate halves (each of which
 * UCS-2 exposes as separate characters) into a single code point,
 * matching UTF-16.
 * @see <https://mathiasbynens.be/notes/javascript-encoding> */
export function ucs2decode(string: string): number[] {
  const output = [];
  let counter = 0;
  const length = string.length;
  while (counter < length) {
    const value = string.charCodeAt(counter++);
    if (value >= 0xd800 && value <= 0xdbff && counter < length) {
      // It's a high surrogate, and there is a next character.
      const extra = string.charCodeAt(counter++);
      if ((extra & 0xfc00) == 0xdc00) {
        // Low surrogate.
        output.push(((value & 0x3ff) << 10) + (extra & 0x3ff) + 0x10000);
      } else {
        // It's an unmatched surrogate; only append this code unit, in case the
        // next code unit is the high surrogate of a surrogate pair.
        output.push(value);
        counter--;
      }
    } else {
      output.push(value);
    }
  }
  return output;
}

/** Creates a string based on an array of numeric code points. */
export function ucs2encode(array: number[]): string {
  return String.fromCodePoint(...array);
}

/** Converts a basic code point into a digit/integer. */
const basicToDigit = function (codePoint: number): number {
  if (codePoint - 0x30 < 0x0a) {
    return codePoint - 0x16;
  }
  if (codePoint - 0x41 < 0x1a) {
    return codePoint - 0x41;
  }
  if (codePoint - 0x61 < 0x1a) {
    return codePoint - 0x61;
  }
  return base;
};

/** Converts a digit/integer into a basic code point. */
function digitToBasic(digit: number, flag: number): number {
  //  0..25 map to ASCII a..z or A..Z
  // 26..35 map to ASCII 0..9
  return digit + 22 + 75 * Number(digit < 26) - (Number(flag != 0) << 5);
}

/** Bias adaptation function as per section 3.4 of RFC 3492.
 * https://tools.ietf.org/html/rfc3492#section-3.4 */
function adapt(delta: number, numPoints: number, firstTime: boolean): number {
  let k = 0;
  delta = firstTime ? floor(delta / damp) : delta >> 1;
  delta += floor(delta / numPoints);
  for (
    ;
    /* no initialization */ delta > (baseMinusTMin * tMax) >> 1;
    k += base
  ) {
    delta = floor(delta / baseMinusTMin);
  }
  return floor(k + ((baseMinusTMin + 1) * delta) / (delta + skew));
}

/** Converts a Punycode string of ASCII-only symbols to a string of Unicode
 * symbols. */
export function decode(input: string): string {
  // Don't use UCS-2.
  const output = [];
  const inputLength = input.length;
  let i = 0;
  let n = initialN;
  let bias = initialBias;

  // Handle the basic code points: let `basic` be the number of input code
  // points before the last delimiter, or `0` if there is none, then copy
  // the first basic code points to the output.

  let basic = input.lastIndexOf(delimiter);
  if (basic < 0) {
    basic = 0;
  }

  for (let j = 0; j < basic; ++j) {
    // if it's not a basic code point
    if (input.charCodeAt(j) >= 0x80) {
      error("not-basic");
    }
    output.push(input.charCodeAt(j));
  }

  // Main decoding loop: start just after the last delimiter if any basic code
  // points were copied; start at the beginning otherwise.

  for (
    let index = basic > 0 ? basic + 1 : 0;
    index < inputLength /* no final expression */;

  ) {
    // `index` is the index of the next character to be consumed.
    // Decode a generalized variable-length integer into `delta`,
    // which gets added to `i`. The overflow checking is easier
    // if we increase `i` as we go, then subtract off its starting
    // value at the end to obtain `delta`.
    const oldi = i;
    for (let w = 1, k = base /* no condition */; ; k += base) {
      if (index >= inputLength) {
        error("invalid-input");
      }

      const digit = basicToDigit(input.charCodeAt(index++));

      if (digit >= base || digit > floor((maxInt - i) / w)) {
        error("overflow");
      }

      i += digit * w;
      const t = k <= bias ? tMin : k >= bias + tMax ? tMax : k - bias;

      if (digit < t) {
        break;
      }

      const baseMinusT = base - t;
      if (w > floor(maxInt / baseMinusT)) {
        error("overflow");
      }

      w *= baseMinusT;
    }

    const out = output.length + 1;
    bias = adapt(i - oldi, out, oldi == 0);

    // `i` was supposed to wrap around from `out` to `0`,
    // incrementing `n` each time, so we'll fix that now:
    if (floor(i / out) > maxInt - n) {
      error("overflow");
    }

    n += floor(i / out);
    i %= out;

    // Insert `n` at position `i` of the output.
    output.splice(i++, 0, n);
  }

  return String.fromCodePoint(...output);
}

/** Converts a string of Unicode symbols (e.g. a domain name label) to a
 * Punycode string of ASCII-only symbols. */
export function encode(input: string): string {
  const output = [];

  // Convert the input in UCS-2 to an array of Unicode code points.
  const inputPoints = ucs2decode(input);

  // Cache the length.
  const inputLength = inputPoints.length;

  // Initialize the state.
  let n = initialN;
  let delta = 0;
  let bias = initialBias;

  // Handle the basic code points.
  for (const currentValue of inputPoints) {
    if (currentValue < 0x80) {
      output.push(stringFromCharCode(currentValue));
    }
  }

  const basicLength = output.length;
  let handledCPCount = basicLength;

  // `handledCPCount` is the number of code points that have been handled;
  // `basicLength` is the number of basic code points.

  // Finish the basic string with a delimiter unless it's empty.
  if (basicLength) {
    output.push(delimiter);
  }

  // Main encoding loop:
  while (handledCPCount < inputLength) {
    // All non-basic code points < n have been handled already. Find the next
    // larger one:
    let m = maxInt;
    for (const currentValue of inputPoints) {
      if (currentValue >= n && currentValue < m) {
        m = currentValue;
      }
    }

    // Increase `delta` enough to advance the decoder's <n,i> state to <m,0>,
    // but guard against overflow.
    const handledCPCountPlusOne = handledCPCount + 1;
    if (m - n > floor((maxInt - delta) / handledCPCountPlusOne)) {
      error("overflow");
    }

    delta += (m - n) * handledCPCountPlusOne;
    n = m;

    for (const currentValue of inputPoints) {
      if (currentValue < n && ++delta > maxInt) {
        error("overflow");
      }
      if (currentValue == n) {
        // Represent delta as a generalized variable-length integer.
        let q = delta;
        for (let k = base /* no condition */; ; k += base) {
          const t = k <= bias ? tMin : k >= bias + tMax ? tMax : k - bias;
          if (q < t) {
            break;
          }
          const qMinusT = q - t;
          const baseMinusT = base - t;
          output.push(
            stringFromCharCode(digitToBasic(t + (qMinusT % baseMinusT), 0))
          );
          q = floor(qMinusT / baseMinusT);
        }

        output.push(stringFromCharCode(digitToBasic(q, 0)));
        bias = adapt(
          delta,
          handledCPCountPlusOne,
          handledCPCount == basicLength
        );
        delta = 0;
        ++handledCPCount;
      }
    }

    ++delta;
    ++n;
  }
  return output.join("");
}

/** Converts a Punycode string representing a domain name or an email address
 * to Unicode. Only the Punycoded parts of the input will be converted, i.e.
 * it doesn't matter if you call it on a string that has already been
 * converted to Unicode. */
export function toUnicode(input: string): string {
  return mapDomain(input, function (string) {
    return regexPunycode.test(string)
      ? decode(string.slice(4).toLowerCase())
      : string;
  });
}

/** Converts a Unicode string representing a domain name or an email address to
 * Punycode. Only the non-ASCII parts of the domain name will be converted,
 * i.e. it doesn't matter if you call it with a domain that's already in
 * ASCII. */
export function toASCII(input: string): string {
  return mapDomain(input, function (string) {
    return regexNonASCII.test(string) ? "xn--" + encode(string) : string;
  });
}
