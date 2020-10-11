# std/mail

## Usage

### Parsing Date

Convert a date field from an email header into a UNIX epoch timestamp. This
function handles the most common formatting of date fields found in email
headers.

```ts
import { parseDate } from "https://deno.land/std@$STD_VERSION/mail/mod.ts";

const timestamp = parseDate("Fri, 30 Nov 2012 20:57:23 GMT");
// ...
```
