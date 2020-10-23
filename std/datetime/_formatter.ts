// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  CallbackResult,
  ReceiverResult,
  Rule,
  TestFunction,
  TestResult,
  Tokenizer,
} from "./_tokenizer.ts";

function digits(value: string | number, count = 2): string {
  return String(value).padStart(count, "0");
}

// as declared as in namespace Intl
type DateTimeFormatPartTypes =
  | "day"
  | "dayPeriod"
  // | "era"
  | "hour"
  | "literal"
  | "minute"
  | "month"
  | "second"
  | "timeZoneName"
  // | "weekday"
  | "year"
  | "fractionalSecond";

interface DateTimeFormatPart {
  type: DateTimeFormatPartTypes;
  value: string;
}

type TimeZone = "UTC";

interface Options {
  timeZone?: TimeZone;
}

function createLiteralTestFunction(value: string): TestFunction {
  return (string: string): TestResult => {
    return string.startsWith(value)
      ? { value, length: value.length }
      : undefined;
  };
}

function createMatchTestFunction(match: RegExp): TestFunction {
  return (string: string): TestResult => {
    const result = match.exec(string);
    if (result) return { value: result, length: result[0].length };
  };
}

function parseTimeZoneOffsets(dateString: string, { value }: FormatPart) {
  const offsets: Array<string | number> = [];
  switch (value) {
    case 1: {
      const [_, hours, minutes = 0] = dateString.match(
        /^([+-]\d{2})(\d{2})?$/,
      )!;
      offsets.push(hours, minutes, 0);
      break;
    }
    case 2: {
      const [_, hours, minutes = 0] = dateString.match(/^([+-]\d{2})(\d{2})$/)!;
      offsets.push(hours, minutes, 0);
      break;
    }
    case 3: {
      const [_, hours, minutes = 0] = dateString.match(
        /^([+-]\d{2})\:(\d{2})$/,
      )!;
      offsets.push(hours, minutes, 0);
      break;
    }
    case 4: {
      const [_, hours, minutes, seconds = 0] = dateString.match(
        /^([+-]\d{2})(\d{2})(\d{2})?$/,
      )!;
      offsets.push(hours, minutes, seconds);
      break;
    }
    case 5: {
      const [_, hours, minutes, seconds = 0] = dateString.match(
        /^([+-]\d{2})\:(\d{2})(?:\:(\d{2}))?$/,
      )!;
      offsets.push(hours, minutes, seconds);
      break;
    }
    default: {
      throw Error(
        `The value "${value}" is not supported.`,
      );
    }
  }
  return offsets.map(Number);
}

// according to unicode symbols (http://www.unicode.org/reports/tr35/tr35-dates.html#Date_Field_Symbol_Table)
const defaultRules = [
  {
    test: createLiteralTestFunction("yyyy"),
    fn: (): CallbackResult => ({
      type: "year",
      value: "numeric",
      format: FormatCase.LowerCase,
    }),
  },
  {
    test: createLiteralTestFunction("yy"),
    fn: (): CallbackResult => ({
      type: "year",
      value: "2-digit",
      format: FormatCase.LowerCase,
    }),
  },

  {
    test: createLiteralTestFunction("MM"),
    fn: (): CallbackResult => ({
      type: "month",
      value: "2-digit",
      format: FormatCase.UpperCase,
    }),
  },
  {
    test: createLiteralTestFunction("M"),
    fn: (): CallbackResult => ({
      type: "month",
      value: "numeric",
      format: FormatCase.UpperCase,
    }),
  },
  {
    test: createLiteralTestFunction("dd"),
    fn: (): CallbackResult => ({
      type: "day",
      value: "2-digit",
      format: FormatCase.LowerCase,
    }),
  },
  {
    test: createLiteralTestFunction("d"),
    fn: (): CallbackResult => ({
      type: "day",
      value: "numeric",
      format: FormatCase.LowerCase,
    }),
  },

  {
    test: createLiteralTestFunction("HH"),
    fn: (): CallbackResult => ({
      type: "hour",
      value: "2-digit",
      format: FormatCase.UpperCase,
    }),
  },
  {
    test: createLiteralTestFunction("H"),
    fn: (): CallbackResult => ({
      type: "hour",
      value: "numeric",
      format: FormatCase.UpperCase,
    }),
  },
  {
    test: createLiteralTestFunction("hh"),
    fn: (): CallbackResult => ({
      type: "hour",
      value: "2-digit",
      format: FormatCase.LowerCase,
    }),
  },
  {
    test: createLiteralTestFunction("h"),
    fn: (): CallbackResult => ({
      type: "hour",
      value: "numeric",
      format: FormatCase.LowerCase,
    }),
  },
  {
    test: createLiteralTestFunction("mm"),
    fn: (): CallbackResult => ({
      type: "minute",
      value: "2-digit",
      format: FormatCase.LowerCase,
    }),
  },
  {
    test: createLiteralTestFunction("m"),
    fn: (): CallbackResult => ({
      type: "minute",
      value: "numeric",
      format: FormatCase.LowerCase,
    }),
  },
  {
    test: createLiteralTestFunction("ss"),
    fn: (): CallbackResult => ({
      type: "second",
      value: "2-digit",
      format: FormatCase.LowerCase,
    }),
  },
  {
    test: createLiteralTestFunction("s"),
    fn: (): CallbackResult => ({
      type: "second",
      value: "numeric",
      format: FormatCase.LowerCase,
    }),
  },
  {
    test: createLiteralTestFunction("SSS"),
    fn: (): CallbackResult => ({
      type: "fractionalSecond",
      value: 3,
      format: FormatCase.UpperCase,
    }),
  },
  {
    test: createLiteralTestFunction("SS"),
    fn: (): CallbackResult => ({
      type: "fractionalSecond",
      value: 2,
      format: FormatCase.UpperCase,
    }),
  },
  {
    test: createLiteralTestFunction("S"),
    fn: (): CallbackResult => ({
      type: "fractionalSecond",
      value: 1,
      format: FormatCase.UpperCase,
    }),
  },

  {
    test: createLiteralTestFunction("a"),
    fn: (value: unknown): CallbackResult => ({
      type: "dayPeriod",
      value: value as string,
      format: FormatCase.LowerCase,
    }),
  },

  {
    test: createLiteralTestFunction("ZZZZZ"),
    fn: (): CallbackResult => ({
      type: "timeZoneName",
      value: 5,
      format: FormatCase.UpperCase,
    }),
  },
  {
    test: createLiteralTestFunction("Z"),
    fn: (): CallbackResult => ({
      type: "timeZoneName",
      value: 1,
      format: FormatCase.UpperCase,
    }),
  },

  // quoted literal
  {
    test: createMatchTestFunction(/^(')(?<value>\\.|[^\']*)\1/),
    fn: (match: unknown): CallbackResult => ({
      type: "literal",
      value: (match as RegExpExecArray).groups!.value as string,
    }),
  },
  // literal
  {
    test: createMatchTestFunction(/^[^a-zA-Z]+?\s*/),
    fn: (match: unknown): CallbackResult => ({
      type: "literal",
      value: (match as RegExpExecArray)[0],
    }),
  },
];

enum FormatCase {
  UpperCase,
  LowerCase,
}

type FormatPart = {
  type: DateTimeFormatPartTypes;
  value: string | number;
  format: FormatCase;
};
type Format = FormatPart[];

export class DateTimeFormatter {
  formatParts: Format;

  constructor(formatString: string, rules: Rule[] = defaultRules) {
    const tokenizer = new Tokenizer(rules);
    this.formatParts = tokenizer.tokenize(
      formatString,
      ({ type, value, format }) => {
        const result = {
          type,
          value,
          format,
        } as unknown as ReceiverResult;
        return result;
      },
    ) as Format;
  }

  format(date: Date): string {
    const timeZoneToken = this.formatParts.find(({ type }) =>
      type === "timeZoneName"
    );
    const utc = timeZoneToken !== undefined;

    let string = "";

    for (const token of this.formatParts) {
      const type = token.type;

      switch (type) {
        case "year": {
          const value = utc ? date.getUTCFullYear() : date.getFullYear();
          switch (token.value) {
            case "numeric": {
              string += value;
              break;
            }
            case "2-digit": {
              string += digits(value, 2).slice(-2);
              break;
            }
            default:
              throw Error(
                `Value "${token.value}" is not supported.`,
              );
          }
          break;
        }
        case "month": {
          const value = (utc ? date.getUTCMonth() : date.getMonth()) + 1;
          switch (token.value) {
            case "numeric": {
              string += value;
              break;
            }
            case "2-digit": {
              string += digits(value, 2);
              break;
            }
            default:
              throw Error(
                `Value "${token.value}" is not supported.`,
              );
          }
          break;
        }
        case "day": {
          const value = utc ? date.getUTCDate() : date.getDate();
          switch (token.value) {
            case "numeric": {
              string += value;
              break;
            }
            case "2-digit": {
              string += digits(value, 2);
              break;
            }
            default:
              throw Error(
                `Value "${token.value}" is not supported.`,
              );
          }
          break;
        }
        case "hour": {
          let value = utc ? date.getUTCHours() : date.getHours();
          value -= token.format === FormatCase.LowerCase && date.getHours() > 12
            ? 12
            : 0;
          switch (token.value) {
            case "numeric": {
              string += value;
              break;
            }
            case "2-digit": {
              string += digits(value, 2);
              break;
            }
            default:
              throw Error(
                `Value "${token.value}" is not supported.`,
              );
          }
          break;
        }
        case "minute": {
          const value = utc ? date.getUTCMinutes() : date.getMinutes();
          switch (token.value) {
            case "numeric": {
              string += value;
              break;
            }
            case "2-digit": {
              string += digits(value, 2);
              break;
            }
            default:
              throw Error(
                `Value "${token.value}" is not supported.`,
              );
          }
          break;
        }
        case "second": {
          const value = utc ? date.getUTCSeconds() : date.getSeconds();
          switch (token.value) {
            case "numeric": {
              string += value;
              break;
            }
            case "2-digit": {
              string += digits(value, 2);
              break;
            }
            default:
              throw Error(
                `Value "${token.value}" is not supported.`,
              );
          }
          break;
        }
        case "fractionalSecond": {
          const value = utc
            ? date.getUTCMilliseconds()
            : date.getMilliseconds();
          string += digits(value, Number(token.value));
          break;
        }
        // FIXME(bartlomieju)
        case "timeZoneName": {
          const offset = date.getTimezoneOffset();
          switch (token.value) {
            case 1: {
              const absOffset = Math.abs(offset);
              const hours = Math.floor(absOffset / 60);
              const minutes = Math.floor(hours / 60);
              const hoursString = `${hours}`.padStart(2, "0");
              const minutesString = `${minutes}`.padStart(2, "0");
              const sign = offset < 0 ? "+" : "-";
              string += `${sign}${hoursString}${minutesString}`;
              break;
            }
            case 5: {
              if (offset === 0) {
                string += "Z";
              } else {
                const absOffset = Math.abs(offset);
                const hours = Math.floor(absOffset / 60);
                const minutes = Math.floor(hours / 60);
                const hoursString = `${hours}`.padStart(2, "0");
                const minutesString = `${minutes}`.padStart(2, "0");
                const sign = offset < 0 ? "+" : "-";
                string += `${sign}${hoursString}:${minutesString}`;
              }
              break;
            }
          }
          break;
        }
        case "dayPeriod": {
          string += token.value ? (date.getHours() >= 12 ? "PM" : "AM") : "";
          break;
        }
        case "literal": {
          string += token.value;
          break;
        }

        default:
          throw Error(`Unexpected token: { ${token.type} ${token.value} }`);
      }
    }

    return string;
  }

  parseToParts(string: string): DateTimeFormatPart[] {
    const parts: DateTimeFormatPart[] = [];
    const initialString = string;

    for (const part of this.formatParts) {
      const type = part.type;

      let value = "";
      switch (part.type) {
        case "year": {
          switch (part.value) {
            case "numeric": {
              value = /^\d{1,4}/.exec(string)?.[0] as string;
              break;
            }
            case "2-digit": {
              value = /^\d{1,2}/.exec(string)?.[0] as string;
              break;
            }
          }
          break;
        }
        case "month": {
          switch (part.value) {
            case "numeric": {
              value = /^1[0-2]|[0-9]/.exec(string)?.[0] as string;
              break;
            }
            case "2-digit": {
              value = /^1[0-2]|0[0-9]/.exec(string)?.[0] as string;
              break;
            }
              // case "narrow": {
              //   value = /^[a-zA-Z]+/.exec(string)?.[0] as string
              //   break
              // }
              // case "short": {
              //   value = /^[a-zA-Z]+/.exec(string)?.[0] as string
              //   break
              // }
              // case "long": {
              //   value = /^[a-zA-Z]+/.exec(string)?.[0] as string
              //   break
              // }
          }
          break;
        }
        case "day": {
          switch (part.value) {
            case "numeric": {
              value = /^\d{1,2}/.exec(string)?.[0] as string;
              break;
            }
            case "2-digit": {
              value = /^\d{2}/.exec(string)?.[0] as string;
              break;
            }
          }
          break;
        }
        case "hour": {
          switch (part.format) {
            case FormatCase.LowerCase: {
              switch (part.value) {
                case "numeric": {
                  value = /^1[0-9]|[0-9]/.exec(string)?.[0] as string;
                  break;
                }
                case "2-digit": {
                  value = /^1[0-9]|0[0-9]/.exec(string)?.[0] as string;
                  break;
                }
              }
              break;
            }
            case FormatCase.UpperCase: {
              switch (part.value) {
                case "numeric": {
                  value = /^2[0-3]|1[0-9]|[0-9]/.exec(string)?.[0] as string;
                  break;
                }
                case "2-digit": {
                  value = /^2[0-3]|1[0-9]|0[0-9]/.exec(string)?.[0] as string;
                  break;
                }
              }
              break;
            }
          }
          break;
        }
        case "minute": {
          switch (part.value) {
            case "numeric": {
              value = /^[0-5][0-9]?|60/.exec(string)?.[0] as string;
              break;
            }
            case "2-digit": {
              value = /^[0-5][0-9]|60/.exec(string)?.[0] as string;
              break;
            }
          }
          break;
        }
        case "second": {
          switch (part.value) {
            case "numeric": {
              value = /^[0-5][0-9]?|60/.exec(string)?.[0] as string;
              break;
            }
            case "2-digit": {
              value = /^[0-5][0-9]|60/.exec(string)?.[0] as string;
              break;
            }
          }
          break;
        }
        case "fractionalSecond": {
          value = new RegExp(`^\\d{${part.value}}`).exec(string)
            ?.[0] as string;
          break;
        }
        case "timeZoneName": {
          if (string.startsWith("Z")) {
            value = "UTC";
          } else {
            switch (part.value) {
              case 1: {
                value = /^[+-](?:[0-1][0-9]|2[0-4])(?:[0-4][0-9]|5[0-9])?/.exec(
                  string,
                )?.[0] as string;
                break;
              }
              case 5: {
                value =
                  /^[+-](?:[0-1][0-9]|2[0-4])\:(?:[0-4][0-9]|5[0-9])(?:\:(?:[0-4][0-9]|5[0-9]))?/
                    .exec(string)
                    ?.[0] as string;
                break;
              }
            }
          }
          break;
        }
        case "dayPeriod": {
          value = /^(A|P)M/.exec(string)?.[0] as string;
          break;
        }
        case "literal": {
          if (!string.startsWith(part.value as string)) {
            throw Error(
              `Literal "${string[0]}" does not match "${part.value}" `,
            );
          }
          value = part.value as string;
          break;
        }
        default:
          throw Error(
            `The part { ${part.type} ${part.value}} is not supported.`,
          );
      }

      if (!value) {
        throw Error(
          `The value for part { ${type} ${part.value} } is invalid.`,
        );
      }
      parts.push({ type, value });

      string = string.slice(value.length);
    }

    if (string.length) {
      throw new Error(
        `Unexpected character at index ${initialString.length - string.length}`,
      );
    }

    return parts;
  }

  partsToDate(parts: DateTimeFormatPart[]): Date {
    const date = new Date();
    const timeZoneToken = parts.find((part) => part.type === "timeZoneName");

    const utc = timeZoneToken?.value === "UTC";
    if (utc) {
      date.setUTCHours(0, 0, 0, 0);
    } else {
      date.setHours(0, 0, 0, 0);
    }

    for (const part of parts) {
      switch (part.type) {
        case "year": {
          const value = Number(part.value.padStart(4, "20"));
          utc ? date.setUTCFullYear(value) : date.setFullYear(value);
          break;
        }
        case "month": {
          const value = Number(part.value) - 1;
          utc ? date.setUTCMonth(value) : date.setMonth(value);
          break;
        }
        case "day": {
          const value = Number(part.value);
          utc ? date.setUTCDate(value) : date.setDate(value);
          break;
        }
        case "hour": {
          let value = Number(part.value);
          const dayPeriod = parts.find(
            (part: DateTimeFormatPart) => part.type === "dayPeriod",
          );
          if (dayPeriod?.value === "PM") value += 12;
          utc ? date.setUTCHours(value) : date.setHours(value);
          break;
        }
        case "minute": {
          const value = Number(part.value);
          utc ? date.setUTCMinutes(value) : date.setMinutes(value);
          break;
        }
        case "second": {
          const value = Number(part.value);
          utc ? date.setUTCSeconds(value) : date.setSeconds(value);
          break;
        }
        case "fractionalSecond": {
          const value = Number(part.value);
          utc ? date.setUTCMilliseconds(value) : date.setMilliseconds(value);
          break;
        }
        case "timeZoneName": {
          break;
        }
        case "dayPeriod": {
          break;
        }
        case "literal": {
          break;
        }
        default:
          throw Error(
            `The part { ${part.type} ${part.value} } is not supported.`,
          );
      }
    }

    if (!utc && timeZoneToken) {
      const localOffset = date.getTimezoneOffset() * 60 * 1000;
      const timeZone = timeZoneToken.value;
      const timeZonePart = this.formatParts.find(({ type }) =>
        type === "timeZoneName"
      )!;
      const [hours, minutes, seconds] = parseTimeZoneOffsets(
        timeZone,
        timeZonePart,
      );
      const offset = (((60 * hours + minutes) * 60) + seconds) * 1000;
      date.setTime(date.getTime() - localOffset - offset);
    }

    return date;
  }

  parse(string: string): Date {
    const parts = this.parseToParts(string);
    return this.partsToDate(parts);
  }
}
