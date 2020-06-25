# flags

Command line arguments parser for Deno based on minimist

# Example

```ts
import { parse } from "https://deno.land/std/flags/mod.ts";

console.dir(parse(Deno.args));
```

```
$ deno run https://deno.land/std/examples/flags.ts -a beep -b boop
{ _: [], a: 'beep', b: 'boop' }
```

```
$ deno run https://deno.land/std/examples/flags.ts -x 3 -y 4 -n5 -abc --beep=boop foo bar baz
{ _: [ 'foo', 'bar', 'baz' ],
  x: 3,
  y: 4,
  n: 5,
  a: true,
  b: true,
  c: true,
  beep: 'boop' }
```

# API

## const parsedArgs = parse(args, options = {});

`parsedArgs._` contains all the arguments that didn't have an option associated
with them.

Numeric-looking arguments will be returned as numbers unless `options.string` or
`options.boolean` is set for that argument name.

Any arguments after `'--'` will not be parsed and will end up in `parsedArgs._`.

options can be:

- `options.string` - a string or array of strings argument names to always treat
  as strings
- `options.boolean` - a boolean, string or array of strings to always treat as
  booleans. if `true` will treat all double hyphenated arguments without equal
  signs as boolean (e.g. affects `--foo`, not `-f` or `--foo=bar`)
- `options.alias` - an object mapping string names to strings or arrays of
  string argument names to use as aliases
- `options.default` - an object mapping string argument names to default values
- `options.stopEarly` - when true, populate `parsedArgs._` with everything after
  the first non-option
- `options['--']` - when true, populate `parsedArgs._` with everything before
  the `--` and `parsedArgs['--']` with everything after the `--`. Here's an
  example:
  ```ts
  // $ deno run example.ts -- a arg1
  import { parse } from "https://deno.land/std/flags/mod.ts";
  console.dir(parse(Deno.args, { "--": false }));
  // output: { _: [ "a", "arg1" ] }
  console.dir(parse(Deno.args, { "--": true }));
  // output: { _: [], --: [ "a", "arg1" ] }
  ```
- `options.unknown` - a function which is invoked with a command line parameter
  not defined in the `options` configuration object. If the function returns
  `false`, the unknown option is not added to `parsedArgs`.
