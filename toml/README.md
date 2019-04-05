# TOML

This module parse TOML files. It follows as much as possible the
[TOML specs](https://github.com/toml-lang/toml). Be sure to read the supported
types as not every specs is supported at the moment and the handling in
TypeScript side is a bit different.

## Supported types and handling

- :heavy_check_mark: [Keys](https://github.com/toml-lang/toml#string)
- :exclamation: [String](https://github.com/toml-lang/toml#string)
- :heavy_check_mark:
  [Multiline String](https://github.com/toml-lang/toml#string)
- :heavy_check_mark: [Literal String](https://github.com/toml-lang/toml#string)
- :exclamation: [Integer](https://github.com/toml-lang/toml#integer)
- :heavy_check_mark: [Float](https://github.com/toml-lang/toml#float)
- :heavy_check_mark: [Boolean](https://github.com/toml-lang/toml#boolean)
- :heavy_check_mark:
  [Offset Date-time](https://github.com/toml-lang/toml#offset-date-time)
- :heavy_check_mark:
  [Local Date-time](https://github.com/toml-lang/toml#local-date-time)
- :heavy_check_mark: [Local Date](https://github.com/toml-lang/toml#local-date)
- :exclamation: [Local Time](https://github.com/toml-lang/toml#local-time)
- :heavy_check_mark: [Table](https://github.com/toml-lang/toml#table)
- :heavy_check_mark: [Inline Table](https://github.com/toml-lang/toml#inline-table)
- :exclamation: [Array of Tables](https://github.com/toml-lang/toml#array-of-tables)

:exclamation: _Supported with warnings see [Warning](#Warning)._

### :warning: Warning

#### String

- Regex : Due to the spec, there is no flag to detect regex properly
  in a TOML declaration. So the regex is stored as string.

#### Integer

For **Binary** / **Octal** / **Hexadecimal** numbers,
they are stored as string to be not interpreted as Decimal.

#### Local Time

Because local time does not exist in JavaScript, the local time is stored as a string.

#### Inline Table

Inline tables are supported. See below:

```toml
animal = { type = { name = "pug" } }
# Output
animal = { type.name = "pug" }
# Output { animal : { type : { name : "pug" } }
animal.as.leaders = "tosin"
# Output { animal: { as: { leaders: "tosin" } } }
"tosin.abasi" = "guitarist"
# Output
"tosin.abasi" : "guitarist"
```

#### Array of Tables

At the moment only simple declarations like below are supported:

```toml
[[bin]]
name = "deno"
path = "cli/main.rs"

[[bin]]
name = "deno_core"
path = "src/foo.rs"

[[nib]]
name = "node"
path = "not_found"
```

will output:

```json
{
  "bin": [
    { "name": "deno", "path": "cli/main.rs" },
    { "name": "deno_core", "path": "src/foo.rs" }
  ],
  "nib": [{ "name": "node", "path": "not_found" }]
}
```

## Usage

### Parse

```ts
import { parseFile, parse } from "./parser.ts";

const tomlObject = parseFile("file.toml");

const tomlString = 'foo.bar = "Deno"';
const tomlObject22 = parse(tomlString);
```

### Stringify

```ts
import { stringify } from "./parser.ts";
const obj = {
  bin: [
    { name: "deno", path: "cli/main.rs" },
    { name: "deno_core", path: "src/foo.rs" }
  ],
  nib: [{ name: "node", path: "not_found" }]
};
const tomlString = stringify(obj);
```
