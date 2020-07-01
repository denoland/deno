# datetime

Simple helper to help parse date strings into `Date`, with additional functions.

## Usage

### parse

- `parse()` - Take an input string and a format to parse the dateTime.

```ts
import { parse } from 'https://deno.land/std/datetime/mod.ts'

parse("20-01-2019", "dd-MM-yyyy") // output : new Date(2019, 0, 20)
parse("2019-01-20", "yyyy-MM-dd") // output : new Date(2019, 0, 20)
parse("2019-01-20", "dd.MM.yyyy") // output : new Date(2019, 0, 20)
parse("01-20-2019 16:34", "MM-dd-yyyy hh:mm") // output : new Date(2019, 0, 20, 16, 34)
parse("01-20-2019 04:34 PM", "MM-dd-yyyy hh:mm a") // output : new Date(2019, 0, 20, 16, 34)
parse("16:34 01-20-2019", "hh:mm MM-dd-yyyy") // output : new Date(2019, 0, 20, 16, 34)
parse("01-20-2019 16:34:23.123", "MM-dd-yyyy hh:mm:ss.SSS") // output : new Date(2019, 0, 20, 16, 34, 23, 123)
...
```

### format

- `format()` - Take an input date and a format to format the dateTime.

```ts
import { format } from 'https://deno.land/std/datetime/mod.ts'

format(new Date(2019, 0, 20), "dd-MM-yyyy") // output : "20-01-2019"
format(new Date(2019, 0, 20), "yyyy-MM-dd") // output : "2019-01-20"
format(new Date(2019, 0, 20), "dd.MM.yyyy") // output : "2019-01-20"
format(new Date(2019, 0, 20, 16, 34), "MM-dd-yyyy hh:mm") // output : "01-20-2019 16:34"
format(new Date(2019, 0, 20, 16, 34), "MM-dd-yyyy hh:mm a") // output : "01-20-2019 04:34 PM"
format(new Date(2019, 0, 20, 16, 34), "hh:mm MM-dd-yyyy") // output : "16:34 01-20-2019"
format(new Date(2019, 0, 20, 16, 34, 23, 123), "MM-dd-yyyy hh:mm:ss.SSS") // output : "01-20-2019 16:34:23.123"
format(new Date(2019, 0, 20), "'today:' yyyy-MM-dd") // output : "today: 2019-01-20"

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
