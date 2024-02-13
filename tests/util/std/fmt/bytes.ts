// Copyright 2014-2021 Sindre Sorhus. All rights reserved. MIT license.
// Copyright 2021 Yoshiya Hinosawa. All rights reserved. MIT license.
// Copyright 2021 Giuseppe Eletto. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** Pretty print bytes.
 *
 * Based on [pretty-bytes](https://github.com/sindresorhus/pretty-bytes).
 * A utility for displaying file sizes for humans.
 *
 * This module is browser compatible.
 *
 * @example
 * ```ts
 * import { format } from "https://deno.land/std@$STD_VERSION/fmt/bytes.ts";
 *
 * format(1337);
 * //=> '1.34 kB'
 *
 * format(100);
 * //=> '100 B'
 *
 * // Display with units of bits
 * format(1337, { bits: true });
 * //=> '1.34 kbit'
 *
 * // Display file size differences
 * format(42, { signed: true });
 * //=> '+42 B'
 *
 * // Localized output using German locale
 * format(1337, { locale: "de" });
 * //=> '1,34 kB'
 * ```
 *
 * @module
 */

type LocaleOptions = {
  minimumFractionDigits?: number;
  maximumFractionDigits?: number;
};

/**
 * The options for pretty printing the byte numbers.
 */
export interface FormatOptions {
  /** Uses bits representation. Default is false. */
  bits?: boolean;
  /** Uses binary bytes (e.g. kibibyte). Default is false. */
  binary?: boolean;
  /** Include plus sign for positive numbers. */
  signed?: boolean;
  /** Uses localized number formatting. If it is set to true, uses default locale on the system. If it's set to string, uses that locale. The given string should be BCP 47 language tag (ref: https://en.wikipedia.org/wiki/IETF_language_tag). You can also give the list of language tags. */
  locale?: boolean | string | string[];
  /** The minimum number of fraction digits to display. If neither minimumFractionDigits or maximumFractionDigits are set, the default behavior is to round to 3 significant digits. */
  minimumFractionDigits?: number;
  /** The maximum number of fraction digits to display. If neither minimumFractionDigits or maximumFractionDigits are set, the default behavior is to round to 3 significant digits. */
  maximumFractionDigits?: number;
}

/**
 * Convert bytes to a human-readable string: 1337 → 1.34 kB
 *
 * @param num The number to format
 * @param options The options
 */
export function format(
  num: number,
  options: FormatOptions = {},
): string {
  if (!Number.isFinite(num)) {
    throw new TypeError(`Expected a finite number, got ${typeof num}: ${num}`);
  }

  const UNITS_FIRSTLETTER = (options.bits ? "b" : "B") + "kMGTPEZY";

  if (options.signed && num === 0) {
    return ` 0 ${UNITS_FIRSTLETTER[0]}`;
  }

  const prefix = num < 0 ? "-" : (options.signed ? "+" : "");
  num = Math.abs(num);

  const localeOptions = getLocaleOptions(options);

  if (num < 1) {
    const numberString = toLocaleString(num, options.locale, localeOptions);
    return prefix + numberString + " " + UNITS_FIRSTLETTER[0];
  }

  const exponent = Math.min(
    Math.floor(
      options.binary ? Math.log(num) / Math.log(1024) : Math.log10(num) / 3,
    ),
    UNITS_FIRSTLETTER.length - 1,
  );
  num /= Math.pow(options.binary ? 1024 : 1000, exponent);

  if (!localeOptions) {
    num = Number(num.toPrecision(3));
  }

  const numberString = toLocaleString(
    num,
    options.locale,
    localeOptions,
  );

  let unit = UNITS_FIRSTLETTER[exponent];
  if (exponent > 0) {
    unit += options.binary ? "i" : "";
    unit += options.bits ? "bit" : "B";
  }

  return prefix + numberString + " " + unit;
}

function getLocaleOptions(
  { maximumFractionDigits, minimumFractionDigits }: FormatOptions,
): LocaleOptions | undefined {
  if (maximumFractionDigits || minimumFractionDigits) {
    return {
      maximumFractionDigits,
      minimumFractionDigits,
    };
  }
}

/**
 * Formats the given number using `Number#toLocaleString`.
 * - If locale is a string, the value is expected to be a locale-key (for example: `de`).
 * - If locale is true, the system default locale is used for translation.
 * - If no value for locale is specified, the number is returned unmodified.
 */
function toLocaleString(
  num: number,
  locale: boolean | string | string[] | undefined,
  options: LocaleOptions | undefined,
): string {
  if (typeof locale === "string" || Array.isArray(locale)) {
    return num.toLocaleString(locale, options);
  } else if (locale === true || options !== undefined) {
    return num.toLocaleString(undefined, options);
  }

  return num.toString();
}
