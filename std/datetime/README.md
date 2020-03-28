# datetime

Simple helper to help parse date strings into `Date`, with additional functions.

## Usage

### parseDate / parseDateTime

- `parseDate()` - Take an input string and a format to parse the date. Supported
  formats are exported in `DateFormat`.
- `parseDateTime()` - Take an input string and a format to parse the dateTime.
  Supported formats are exported in `DateTimeFormat`.

```ts
import { parseDate, parseDateTime } from 'https://deno.land/std/datetime/mod.ts'

parseDate("20-01-2019", "dd-mm-yyyy") // output : new Date(2019, 0, 20)
parseDate("2019-01-20", "yyyy-mm-dd") // output : new Date(2019, 0, 20)
...

parseDateTime("01-20-2019 16:34", "mm-dd-yyyy hh:mm") // output : new Date(2019, 0, 20, 16, 34)
parseDateTime("16:34 01-20-2019", "hh:mm mm-dd-yyyy") // output : new Date(2019, 0, 20, 16, 34)
...
```

### dayOfYear / currentDayOfYear

- `dayOfYear()` - Returns the number of the day in the year.
- `currentDayOfYear()` - Returns the number of the current day in the year.

```ts
import {
  dayOfYear,
  currentDayOfYear,
} from "https://deno.land/std/datetime/mod.ts";

dayOfYear(new Date("2019-03-11T03:24:00")); // output: 70
currentDayOfYear(); // output: ** depends on when you run it :) **
```
