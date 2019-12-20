# Encoding

## CSV

- **`readAll(reader: BufReader, opt: ParseOptions = { comma: ",", trimLeadingSpace: false, lazyQuotes: false } ): Promise<[string[][], BufState]>`**:
  Read the whole buffer and output the structured CSV datas
- **`parse(csvString: string, opt: ParseOption): Promise<unknown[]>`**: See
  [parse](###Parse)

### Parse

Parse the CSV string with the options provided.

#### Options

##### ParseOption

- **`header: boolean | string[] | HeaderOption[];`**: If a boolean is provided,
  the first line will be used as Header definitions. If `string[]` or
  `HeaderOption[]` those names will be used for header definition.
- **`parse?: (input: unknown) => unknown;`**: Parse function for the row, which
  will be executed after parsing of all columns. Therefore if you don't provide
  header and parse function with headers, input will be `string[]`.

##### HeaderOption

- **`name: string;`**: Name of the header to be used as property.
- **`parse?: (input: string) => unknown;`**: Parse function for the column. This
  is executed on each entry of the header. This can be combined with the Parse
  function of the rows.

#### Usage

```ts
// input:
// a,b,c
// e,f,g

const r = await parseFile(filepath, {
  header: false
});
// output:
// [["a", "b", "c"], ["e", "f", "g"]]

const r = await parseFile(filepath, {
  header: true
});
// output:
// [{ a: "e", b: "f", c: "g" }]

const r = await parseFile(filepath, {
  header: ["this", "is", "sparta"]
});
// output:
// [
//   { this: "a", is: "b", sparta: "c" },
//   { this: "e", is: "f", sparta: "g" }
// ]

const r = await parseFile(filepath, {
  header: [
    {
      name: "this",
      parse: (e: string): string => {
        return `b${e}$$`;
      }
    },
    {
      name: "is",
      parse: (e: string): number => {
        return e.length;
      }
    },
    {
      name: "sparta",
      parse: (e: string): unknown => {
        return { bim: `boom-${e}` };
      }
    }
  ]
});
// output:
// [
//    { this: "ba$$", is: 1, sparta: { bim: `boom-c` } },
//    { this: "be$$", is: 1, sparta: { bim: `boom-g` } }
// ]

const r = await parseFile(filepath, {
  header: ["this", "is", "sparta"],
  parse: (e: Record<string, unknown>) => {
    return { super: e.this, street: e.is, fighter: e.sparta };
  }
});
// output:
// [
//   { super: "a", street: "b", fighter: "c" },
//   { super: "e", street: "f", fighter: "g" }
// ]
```

## TOML

This module parse TOML files. It follows as much as possible the
[TOML specs](https://github.com/toml-lang/toml). Be sure to read the supported
types as not every specs is supported at the moment and the handling in
TypeScript side is a bit different.

### Supported types and handling

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
- :heavy_check_mark:
  [Inline Table](https://github.com/toml-lang/toml#inline-table)
- :exclamation:
  [Array of Tables](https://github.com/toml-lang/toml#array-of-tables)

:exclamation: _Supported with warnings see [Warning](#Warning)._

#### :warning: Warning

##### String

- Regex : Due to the spec, there is no flag to detect regex properly in a TOML
  declaration. So the regex is stored as string.

##### Integer

For **Binary** / **Octal** / **Hexadecimal** numbers, they are stored as string
to be not interpreted as Decimal.

##### Local Time

Because local time does not exist in JavaScript, the local time is stored as a
string.

##### Inline Table

Inline tables are supported. See below:

```toml
animal = { type = { name = "pug" } }
## Output
animal = { type.name = "pug" }
## Output { animal : { type : { name : "pug" } }
animal.as.leaders = "tosin"
## Output { animal: { as: { leaders: "tosin" } } }
"tosin.abasi" = "guitarist"
## Output
"tosin.abasi" : "guitarist"
```

##### Array of Tables

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

### Usage

#### Parse

```ts
import { parse } from "./parser.ts";
import { readFileStrSync } from "../fs/read_file_str.ts";

const tomlObject = parse(readFileStrSync("file.toml"));

const tomlString = 'foo.bar = "Deno"';
const tomlObject22 = parse(tomlString);
```

#### Stringify

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

## YAML

YAML parser / dumper for Deno

Heavily inspired from [js-yaml]

### Basic usage

`parse` parses the yaml string, and `stringify` dumps the given object to YAML
string.

```ts
import { parse, stringify } from "https://deno.land/std/encoding/yaml.ts";

const data = parse(`
foo: bar
baz:
  - qux
  - quux
`);
console.log(data);
// => { foo: "bar", baz: [ "qux", "quux" ] }

const yaml = stringify({ foo: "bar", baz: ["qux", "quux"] });
console.log(yaml);
// =>
// foo: bar
// baz:
//   - qux
//   - quux
```

If your YAML contains multiple documents in it, you can use `parseAll` for
handling it.

```ts
import { parseAll } from "https://deno.land/std/encoding/yaml.ts";

const data = parseAll(`
---
id: 1
name: Alice
---
id: 2
name: Bob
---
id: 3
name: Eve
`);
console.log(data);
// => [ { id: 1, name: "Alice" }, { id: 2, name: "Bob" }, { id: 3, name: "Eve" } ]
// TODO(kt3k): This doesn't work now
```

### API

#### `parse(str: string, opts?: ParserOption): unknown`

Parses the YAML string with a single document.

#### `parseAll(str: string, iterator?: Function, opts?: ParserOption): unknown`

Parses the YAML string with multiple documents. If the iterator is given, it's
applied to every document instead of returning the array of parsed objects.

#### `stringify(obj: object, opts?: DumpOption): string`

Serializes `object` as a YAML document.

### :warning: Limitations

- `binary` type is currently not stable
- `function`, `regexp`, and `undefined` type are currently not supported

### More example

See [`./yaml/example`](./yaml/example) folder and [js-yaml] repository.

[js-yaml]: https://github.com/nodeca/js-yaml
