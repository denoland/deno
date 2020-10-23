# datetime

Simple helper to help parse date strings into `Date`, with additional functions.

## Usage

The following symbols from
[unicode LDML](http://www.unicode.org/reports/tr35/tr35-dates.html#Date_Field_Symbol_Table)
are supported:

|       Symbol | Description               | Example              |
| -----------: | ------------------------- | -------------------- |
|           yy | 2-digit year              | 1, 01                |
|         yyyy | numeric year              | 2001                 |
|            M | numeric month             | 2, 02                |
|           MM | 2-digit month             | 02                   |
|            d | numeric month             | 3, 03                |
|           dd | 2-digit month             | 03                   |
|            H | numeric hour (0-23 hours) | 4, 04                |
|           HH | 2-digit hour (0-23 hours) | 23                   |
|            h | numeric hour (1-12 hours) | 5, 05                |
|           hh | 2-digit hour (1-12 hours) | 11                   |
|            m | numeric minute            | 6, 06                |
|           mm | 2-digit minute            | 06                   |
|            s | numeric second            | 7, 07                |
|           ss | 2-digit second            | 07                   |
|            S | 1-digit fractional second | 8                    |
|           SS | 2-digit fractional second | 08                   |
|          SSS | 3-digit fractional second | 008                  |
|            Z | timezone                  | -0800                |
|        ZZZZZ | timezone                  | Z, -08:00, -07:30:58 |
|            a | day period                | AM, PM               |
| '\<literal>' | literal                   | 'foo'                |
|  <separator> | separator                 | ./-                  |
|              |                           |                      |

## Functions

### parse

Takes an input `string` and a `formatString` to parse to a `date`.

```ts
import { parse } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

parse("22-10-1993", "dd-MM-yyyy"); // Date("1993-10-22")
```

### format

Takes an input `date` and a `formatString` to format to a `string`.

```ts
import { format } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

format(new Date("1993-10-22"), "dd-MM-yyyy"); // "22.10.1993"
```

### dayOfYear

Returns the number of the day in the year.

```ts
import { dayOfYear } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

dayOfYear(new Date("2019-03-11T03:24:00")); // output: 70
```

### weekOfYear

Returns the ISO week number of the provided date (1-53).

```ts
import { weekOfYear } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

weekOfYear(new Date("2020-12-28T03:24:00")); // Returns 53
```

### toIMF

Formats the given date to IMF date time format. (Reference:
https://tools.ietf.org/html/rfc7231#section-7.1.1.1 )

```js
import { toIMF } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

toIMF(new Date(0)); // "Thu, 01 Jan 1970 00:00:00 GMT"
```

### isLeap

Returns true if the given date or year (in number) is a leap year. Returns false
otherwise.

```js
import { isLeap } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

isLeap(new Date("1970-01-01")); // false
isLeap(1972); // returns true
```

### difference

Returns the difference of the 2 given dates in the given units. If the units are
omitted, it returns the difference in the all available units.

Available units:

- `"milliseconds"`
- `"seconds"`
- `"minutes"`
- `"hours"`
- `"days"`
- `"weeks",`
- `"months"`
- `"quarters"`
- `"years"`

```js
import { difference } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

difference(
  new Date("2018-05-14"),
  new Date("2020-05-13"),
  { units: ["days", "months", "years"] },
); // { days: 730, months: 23, years: 1 }
```

## Constants

|      Constant | Description                        | Value     |
| ------------: | ---------------------------------- | --------- |
|        SECOND | number of milliseconds in a second | 1000      |
|        MINUTE | number of milliseconds in a minute | 60000     |
|          HOUR | number of milliseconds in a hour   | 3600000   |
|           DAY | number of milliseconds in a day    | 86400000  |
|          WEEK | number of milliseconds in a week   | 604800000 |
| DAYS_PER_WEEK | number of days in a week           | 7         |
