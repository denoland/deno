// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { Tokenizer, Rule } from "./Tokenizer.ts"

function digits(value: any, count = 2): string { return String(value).padStart(count, "0") }

// as declared in namespace Intl
type DateTimeFormatPartTypes = "day" | "dayPeriod" | "era" | "hour" | "literal" | "minute" | "month" | "second" | "timeZoneName" | "weekday" | "year" | "fractionalSecond"

interface DateTimeFormatPart {
  type: DateTimeFormatPartTypes
  value: string
}

type TimeZone = "UTC"

export interface Options {
  timeZone?: TimeZone
}

// according to unicode symbols (http://userguide.icu-project.org/formatparse/datetime)
const defaultRules = [
  { test: /^yyyy/, fn: (match: any) => ({ type: "year", value: "numeric" }) },
  { test: /^yy/, fn: (match: any) => ({ type: "year", value: "2-digit" }) },

  { test: /^MM/, fn: (match: any) => ({ type: "month", value: "2-digit" }) },
  { test: /^M/, fn: (match: any) => ({ type: "month", value: "numeric" }) },
  { test: /^dd/, fn: (match: any) => ({ type: "day", value: "2-digit" }) },
  { test: /^d/, fn: (match: any) => ({ type: "day", value: "numeric" }) },

  { test: /^hh/, fn: (match: any) => ({ type: "hour", value: "2-digit" }) },
  { test: /^h/, fn: (match: any) => ({ type: "hour", value: "numeric" }) },
  { test: /^mm/, fn: (match: any) => ({ type: "minute", value: "2-digit" }) },
  { test: /^m/, fn: (match: any) => ({ type: "minute", value: "numeric" }) },
  { test: /^ss/, fn: (match: any) => ({ type: "second", value: "2-digit" }) },
  { test: /^s/, fn: (match: any) => ({ type: "second", value: "numeric" }) },
  { test: /^SSS/, fn: (match: any) => ({ type: "fractionalSecond", value: 3 }) },
  { test: /^SS/, fn: (match: any) => ({ type: "fractionalSecond", value: 2 }) },
  { test: /^S/, fn: (match: any) => ({ type: "fractionalSecond", value: 1 }) },

  { test: /^a/, fn: (match: any) => ({ type: "dayPeriod", value: match[0] }) },

  // quoted literal
  { test: /^(')(?<value>\\.|[^\']*)\1/, fn: (match: any) => ({ type: "literal", value: match.groups.value }) },
  // literal
  { test: /^.+?\s*/, fn: (match: any) => ({ type: "literal", value: match[0] }) },

]

export class DateTimeFormatter {
  #format: any

  constructor(formatString: string, rules: Rule[] = defaultRules) {
    const tokenizer = new Tokenizer(rules)
    this.#format = tokenizer.tokenize(formatString, ({ type, value }) => ({ type, value }))
  }

  format(date: Date, options: Options = {}) {
    let string = ""

    const utc = options.timeZone === "UTC"
    const hour12 = this.#format.find((token: any) => token.type === "dayPeriod")

    for (const token of this.#format) {
      const type = token.type

      switch (type) {
        case "year": {
          const value = utc ? date.getUTCFullYear() : date.getFullYear()
          switch (token.value) {
            case "numeric": {
              string += digits(value, 4)
              break
            }
            case "2-digit": {
              string += digits(value, 2).slice(-2)
              break
            }
            default: throw Error(`FormatterError: value "${token.value}" is not supported`)
          }
          break
        }
        case "month": {
          const value = (utc ? date.getUTCMonth() : date.getMonth()) + 1
          switch (token.value) {
            case "numeric": {
              string += value
              break
            }
            case "2-digit": {
              string += digits(value, 2)
              break
            }
            default: throw Error(`FormatterError: value "${token.value}" is not supported`)
          }
          break
        }
        case "day": {
          const value = utc ? date.getUTCDate() : date.getDate()
          switch (token.value) {
            case "numeric": {
              string += value
              break
            }
            case "2-digit": {
              string += digits(value, 2)
              break
            }
            default: throw Error(`FormatterError: value "${token.value}" is not supported`)
          }
          break
        }
        case "hour": {
          let value = utc ? date.getUTCHours() : date.getHours()
          value -= hour12 && date.getHours() > 12 ? 12 : 0
          switch (token.value) {
            case "numeric": {
              string += value
              break
            }
            case "2-digit": {
              string += digits(value, 2)
              break
            }
            default: throw Error(`FormatterError: value "${token.value}" is not supported`)
          }
          break
        }
        case "minute": {
          const value = utc ? date.getUTCMinutes() : date.getMinutes()
          switch (token.value) {
            case "numeric": {
              string += value
              break
            }
            case "2-digit": {
              string += digits(value, 2)
              break
            }
            default: throw Error(`FormatterError: value "${token.value}" is not supported`)
          }
          break
        }
        case "second": {
          const value = utc ? date.getUTCSeconds() : date.getSeconds()
          switch (token.value) {
            case "numeric": {
              string += value
              break
            }
            case "2-digit": {
              string += digits(value, 2)
              break
            }
            default: throw Error(`FormatterError: value "${token.value}" is not supported`)
          }
          break
        }
        case "fractionalSecond": {
          const value = utc ? date.getUTCMilliseconds() : date.getMilliseconds()
          string += digits(value, token.value)
          break
        }
        case "timeZoneName": {
          // string += utc ? "Z" : token.value
          // break
        }
        case "dayPeriod": {
          string += hour12 ? date.getHours() >= 12 ? "PM" : "AM" : ""
          break
        }
        case "literal": {
          string += token.value
          break
        }

        default: throw Error(`FormatterError: { ${token.type} ${token.value} }`)

      }
    }

    return string
  }

  parseToParts(string: string) {
    const parts: DateTimeFormatPart[] = []

    for (const token of this.#format) {
      const type = token.type

      let value
      switch (token.type) {
        case "year": {
          switch (token.value) {
            case "numeric": {
              value = /^\d{1,4}/.exec(string)?.[0]
              break
            }
            case "2-digit": {
              value = /^\d{1,2}/.exec(string)?.[0]
              break
            }
          }
          break
        }
        case "month": {
          switch (token.value) {
            case "numeric": {
              value = /^\d{1,2}/.exec(string)?.[0]
              break
            }
            case "2-digit": {
              value = /^\d{2}/.exec(string)?.[0]
              break
            }
            case "narrow": {
              value = /^[a-zA-Z]+/.exec(string)?.[0]
              break
            }
            case "short": {
              value = /^[a-zA-Z]+/.exec(string)?.[0]
              break
            }
            case "long": {
              value = /^[a-zA-Z]+/.exec(string)?.[0]
              break
            }
            default: throw Error(`ParserError: value "${token.value}" is not supported`)
          }
          break
        }
        case "day": {
          switch (token.value) {
            case "numeric": {
              value = /^\d{1,2}/.exec(string)?.[0]
              break
            }
            case "2-digit": {
              value = /^\d{2}/.exec(string)?.[0]
              break
            }
            default: throw Error(`ParserError: value "${token.value}" is not supported`)
          }
          break
        }
        case "hour": {
          switch (token.value) {
            case "numeric": {
              value = /^\d{1,2}/.exec(string)?.[0]
              break
            }
            case "2-digit": {
              value = /^\d{2}/.exec(string)?.[0]
              break
            }
            default: throw Error(`ParserError: value "${token.value}" is not supported`)
          }
          break
        }
        case "minute": {
          switch (token.value) {
            case "numeric": {
              value = /^\d{1,2}/.exec(string)?.[0]
              break
            }
            case "2-digit": {
              value = /^\d{2}/.exec(string)?.[0]
              break
            }
            default: throw Error(`ParserError: value "${token.value}" is not supported`)
          }
          break
        }
        case "second": {
          switch (token.value) {
            case "numeric": {
              value = /^\d{1,2}/.exec(string)?.[0]
              break
            }
            case "2-digit": {
              value = /^\d{2}/.exec(string)?.[0]
              break
            }
            default: throw Error(`ParserError: value "${token.value}" is not supported`)
          }
          break
        }
        case "fractionalSecond": {
          value = new RegExp(`^\\d{${token.value}}`).exec(string)?.[0]
          break
        }
        case "timeZoneName": {
          value = token.value
          break
        }
        case "dayPeriod": {
          value = /^(A|P)M/.exec(string)?.[0]
          break
        }
        case "literal": {
          if (!string.startsWith(token.value)) { throw Error(`Literal "${token.value}" not found "${string.slice(0, 25)}"`) }
          value = token.value
          break
        }

        default: throw Error(`${token.type} ${token.value}`)

      }

      parts.push({ type, value })
      if (!value) {
        throw Error(`value not valid for token { ${type} ${value} } ${string.slice(0, 25)}`)
      }
      string = string.slice(value.length)
    }

    if (string.length) { throw Error(`datetime string was not fully parsed! ${string.slice(0, 25)}`) }

    return parts
  }

  partsToDate(parts: DateTimeFormatPart[]) {
    const date = new Date()
    const utc = parts.find(part => part.type === "timeZoneName" && part.value === "UTC")

    utc ? date.setUTCHours(0, 0, 0, 0) : date.setHours(0, 0, 0, 0)
    for (const part of parts) {
      switch (part.type) {
        case "year": {
          const value = Number(part.value.padStart(4, "20"))
          utc ? date.setUTCFullYear(value) : date.setFullYear(value)
          break
        }
        case "month": {
          const value = Number(part.value) - 1
          utc ? date.setUTCMonth(value) : date.setMonth(value)
          break
        }
        case "day": {
          const value = Number(part.value)
          utc ? date.setUTCDate(value) : date.setDate(value)
          break
        }
        case "hour": {
          let value = Number(part.value)
          const dayPeriod = parts.find((part: any) => part.type === "dayPeriod")
          if (dayPeriod?.value === "PM") { value += 12 }
          utc ? date.setUTCHours(value) : date.setHours(value)
          break
        }
        case "minute": {
          const value = Number(part.value)
          utc ? date.setUTCMinutes(value) : date.setMinutes(value)
          break
        }
        case "second": {
          const value = Number(part.value)
          utc ? date.setUTCSeconds(value) : date.setSeconds(value)
          break
        }
        case "fractionalSecond": {
          const value = Number(part.value)
          utc ? date.setUTCMilliseconds(value) : date.setMilliseconds(value)
          break
        }
      }
    }
    return date
  }

  parse(string: string) {
    const parts = this.parseToParts(string)
    return this.partsToDate(parts)
  }

}

export const SECOND = 1e3
export const MINUTE = SECOND * 60
export const HOUR = MINUTE * 60
export const DAY = HOUR * 24
export const WEEK = DAY * 7

/**
 * Parse date from string using format string
 * @param dateString Date string
 * @param format Format string
 * @return Parsed date
 */
export function parse(dateString: string, formatString: string): Date {
  const formatter = new DateTimeFormatter(formatString)
  const parts = formatter.parseToParts(dateString)
  return formatter.partsToDate(parts)
}

/**
 * Format date using format string
 * @param date Date
 * @param format Format string
 * @return formatted date string
 */
export function format(date: Date, formatString: string): string {
  const formatter = new DateTimeFormatter(formatString)
  return formatter.format(date)
}

/**
 * Get number of the day in the year
 * @return Number of the day in year
 */
export function dayOfYear(date: Date): number {
  const yearStart = new Date(date.getFullYear(), 0, 0)
  const diff =
    date.getTime() -
    yearStart.getTime() +
    (yearStart.getTimezoneOffset() - date.getTimezoneOffset()) * 60 * 1000
  return Math.floor(diff / DAY)
}

/**
 * Get number of current day in year
 * @return Number of current day in year
 */
export function currentDayOfYear(): number {
  return dayOfYear(new Date())
}

/**
 * Parse a date to return a IMF formated string date
 * RFC: https://tools.ietf.org/html/rfc7231#section-7.1.1.1
 * IMF is the time format to use when generating times in HTTP
 * headers. The time being formatted must be in UTC for Format to
 * generate the correct format.
 * @param date Date to parse
 * @return IMF date formated string
 */
export function toIMF(date: Date): string {
  function dtPad(v: string, lPad = 2): string {
    return v.padStart(lPad, "0")
  }
  const d = dtPad(date.getUTCDate().toString())
  const h = dtPad(date.getUTCHours().toString())
  const min = dtPad(date.getUTCMinutes().toString())
  const s = dtPad(date.getUTCSeconds().toString())
  const y = date.getUTCFullYear()
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
  const months = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ]
  return `${days[date.getUTCDay()]}, ${d} ${
    months[date.getUTCMonth()]
    } ${y} ${h}:${min}:${s} GMT`
}

/**
 * Check given year is a leap year or not.
 * based on : https://docs.microsoft.com/en-us/office/troubleshoot/excel/determine-a-leap-year
 * @param year year in number or Date format
 */
export function isLeap(year: Date | number): boolean {
  const yearNumber = year instanceof Date ? year.getFullYear() : year
  return (
    (yearNumber % 4 === 0 && yearNumber % 100 !== 0) || yearNumber % 400 === 0
  )
}

export type Unit =
  | "miliseconds"
  | "seconds"
  | "minutes"
  | "hours"
  | "days"
  | "weeks"
  | "months"
  | "quarters"
  | "years"

export type DifferenceFormat = Partial<Record<Unit, number>>

export type DifferenceOptions = {
  units?: Unit[]
}

/**
 * Calculate difference between two dates.
 * @param from Year to calculate difference
 * @param to Year to calculate difference with
 * @param options Options for determining how to respond
 *
 * example :
 *
 * ```typescript
 * datetime.difference(new Date("2020/1/1"),new Date("2020/2/2"),{ units : ["days","months"] })
 * ```
 */
export function difference(
  from: Date,
  to: Date,
  options?: DifferenceOptions
): DifferenceFormat {
  const uniqueUnits = options?.units
    ? [...new Set(options?.units)]
    : [
      "miliseconds",
      "seconds",
      "minutes",
      "hours",
      "days",
      "weeks",
      "months",
      "quarters",
      "years",
    ]

  const bigger = Math.max(from.getTime(), to.getTime())
  const smaller = Math.min(from.getTime(), to.getTime())
  const differenceInMs = bigger - smaller

  const differences: DifferenceFormat = {}

  for (const uniqueUnit of uniqueUnits) {
    switch (uniqueUnit) {
      case "miliseconds":
        differences.miliseconds = differenceInMs
        break
      case "seconds":
        differences.seconds = Math.floor(differenceInMs / SECOND)
        break
      case "minutes":
        differences.minutes = Math.floor(differenceInMs / MINUTE)
        break
      case "hours":
        differences.hours = Math.floor(differenceInMs / HOUR)
        break
      case "days":
        differences.days = Math.floor(differenceInMs / DAY)
        break
      case "weeks":
        differences.weeks = Math.floor(differenceInMs / WEEK)
        break
      case "months":
        differences.months = calculateMonthsDifference(bigger, smaller)
        break
      case "quarters":
        differences.quarters = Math.floor(
          (typeof differences.months !== "undefined" &&
            differences.months / 4) ||
          calculateMonthsDifference(bigger, smaller) / 4
        )
        break
      case "years":
        differences.years = Math.floor(
          (typeof differences.months !== "undefined" &&
            differences.months / 12) ||
          calculateMonthsDifference(bigger, smaller) / 12
        )
        break
    }
  }

  return differences
}

function calculateMonthsDifference(bigger: number, smaller: number): number {
  const biggerDate = new Date(bigger)
  const smallerDate = new Date(smaller)
  const yearsDiff = biggerDate.getFullYear() - smallerDate.getFullYear()
  const monthsDiff = biggerDate.getMonth() - smallerDate.getMonth()
  const calendarDiffrences = Math.abs(yearsDiff * 12 + monthsDiff)
  const compareResult = biggerDate > smallerDate ? 1 : -1
  biggerDate.setMonth(
    biggerDate.getMonth() - compareResult * calendarDiffrences
  )
  const isLastMonthNotFull =
    biggerDate > smallerDate ? 1 : -1 === -compareResult ? 1 : 0
  const months = compareResult * (calendarDiffrences - isLastMonthNotFull)
  return months === 0 ? 0 : months
}
