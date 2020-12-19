# datetime

Simple helper to help parse date strings into `Date`, with additional functions.

## Usage

The following symbols from
[unicode LDML](http://www.unicode.org/reports/tr35/tr35-dates.html#Date_Field_Symbol_Table)
are supported:

- `yyyy` - numeric year.
- `yy` - 2-digit year.
- `M` - numeric month.
- `MM` - 2-digit month.
- `d` - numeric day.
- `dd` - 2-digit day.

- `H` - numeric hour (0-23 hours).
- `HH` - 2-digit hour (00-23 hours).
- `h` - numeric hour (1-12 hours).
- `hh` - 2-digit hour (01-12 hours).
- `m` - numeric minute.
- `mm` - 2-digit minute.
- `s` - numeric second.
- `ss` - 2-digit second.
- `S` - 1-digit fractionalSecond.
- `SS` - 2-digit fractionalSecond.
- `SSS` - 3-digit fractionalSecond.

- `a` - dayPeriod, either `AM` or `PM`.

- `'foo'` - quoted literal.
- `./-` - unquoted literal.

## Methods

### parse

Takes an input `string` and a `formatString` to parse to a `date`.

```ts
import { parse } from 'https://deno.land/std@0.69.0/datetime/mod.ts'

parse("20-01-2019", "dd-MM-yyyy") // output : new Date(2019, 0, 20)
parse("2019-01-20", "yyyy-MM-dd") // output : new Date(2019, 0, 20)
parse("2019-01-20", "dd.MM.yyyy") // output : new Date(2019, 0, 20)
parse("01-20-2019 16:34", "MM-dd-yyyy HH:mm") // output : new Date(2019, 0, 20, 16, 34)
parse("01-20-2019 04:34 PM", "MM-dd-yyyy hh:mm a") // output : new Date(2019, 0, 20, 16, 34)
parse("16:34 01-20-2019", "HH:mm MM-dd-yyyy") // output : new Date(2019, 0, 20, 16, 34)
parse("01-20-2019 16:34:23.123", "MM-dd-yyyy HH:mm:ss.SSS") // output : new Date(2019, 0, 20, 16, 34, 23, 123)
...
```

### format

Takes an input `date` and a `formatString` to format to a `string`.

```ts
import { format } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

format(new Date(2019, 0, 20), "dd-MM-yyyy"); // output : "20-01-2019"
format(new Date(2019, 0, 20), "yyyy-MM-dd"); // output : "2019-01-20"
format(new Date(2019, 0, 20), "dd.MM.yyyy"); // output : "2019-01-20"
format(new Date(2019, 0, 20, 16, 34), "MM-dd-yyyy HH:mm"); // output : "01-20-2019 16:34"
format(new Date(2019, 0, 20, 16, 34), "MM-dd-yyyy hh:mm a"); // output : "01-20-2019 04:34 PM"
format(new Date(2019, 0, 20, 16, 34), "HH:mm MM-dd-yyyy"); // output : "16:34 01-20-2019"
format(new Date(2019, 0, 20, 16, 34, 23, 123), "MM-dd-yyyy HH:mm:ss.SSS"); // output : "01-20-2019 16:34:23.123"
format(new Date(2019, 0, 20), "'today:' yyyy-MM-dd"); // output : "today: 2019-01-20"
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

toIMF(new Date(0)); // => returns "Thu, 01 Jan 1970 00:00:00 GMT"
```

### isLeap

Returns true if the given date or year (in number) is a leap year. Returns false
otherwise.

```js
import { isLeap } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

isLeap(new Date("1970-01-01")); // => returns false
isLeap(new Date("1972-01-01")); // => returns true
isLeap(new Date("2000-01-01")); // => returns true
isLeap(new Date("2100-01-01")); // => returns false
isLeap(1972); // => returns true
```

### difference

Returns the difference of the 2 given dates in the given units. If the units are
omitted, it returns the difference in the all available units.

Available units: "milliseconds", "seconds", "minutes", "hours", "days", "weeks",
"months", "quarters", "years"

```js
import { difference } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

const date0 = new Date("2018-05-14");
const date1 = new Date("2020-05-13");

difference(date0, date1, { units: ["days", "months", "years"] });
// => returns { days: 730, months: 23, years: 1 }

difference(date0, date1);
// => returns {
//   milliseconds: 63072000000,
//   seconds: 63072000,
//   minutes: 1051200,
//   hours: 17520,
//   days: 730,
//   weeks: 104,
//   months: 23,
//   quarters: 5,
//   years: 1
// }
```

## Constants

### SECOND

```
import { SECOND } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

console.log(SECOND); // => 1000
```

### MINUTE

```
import { MINUTE } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

console.log(MINUTE); // => 60000 (60 * 1000)
```

### HOUR

```
import { HOUR } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

console.log(HOUR); // => 3600000 (60 * 60 * 1000)
```

### DAY

```
import { DAY } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

console.log(DAY); // => 86400000 (24 * 60 * 60 * 1000)
```

### WEEK

```
import { WEEK } from "https://deno.land/std@$STD_VERSION/datetime/mod.ts";

console.log(WEEK); // => 604800000 (7 * 24 * 60 * 60 * 1000)
```
