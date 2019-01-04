# colors

Is a basic console color module intended for [Deno](https://deno.land/). It is
inspired by [chalk](https://www.npmjs.com/package/chalk) and
[colors](https://www.npmjs.com/package/colors) on npm.

## Usage

The main modules exports a single function name `color` which is a function that
provides chaining to stack colors. Basic usage looks like this:

```ts
import { color } from "https://deno.land/x/colors/main.ts";

console.log(color.bgBlue.red.bold("Hello world!"));
```

## TODO

- Currently, it just assumes it is running in an environment that supports ANSI
  escape code terminal coloring. It should actually detect, specifically windows
  and adjust properly.

- Test coverage is very basic at the moment.

---

Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
