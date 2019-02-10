# colors

Is a basic console color module intended for [Deno](https://deno.land/). It is
inspired by [chalk](https://www.npmjs.com/package/chalk),
[kleur](https://www.npmjs.com/package/kleur), and
[colors](https://www.npmjs.com/package/colors) on npm.

## Usage

The main modules exports several functions which can color the output to the
console:

```ts
import { bgBlue, red, bold } from "https://deno.land/x/std/colors/mod.ts";

console.log(bgBlue(red(bold("Hello world!"))));
```

This module supports `NO_COLOR` environmental variable disabling any coloring if `NO_COLOR` is set.

## TODO

- Currently, it just assumes it is running in an environment that supports ANSI
  escape code terminal coloring. It should actually detect, specifically windows
  and adjust properly.

- Test coverage is very basic at the moment.

---

Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
