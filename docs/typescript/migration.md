## Migrating to and from JavaScript

One of the advantages of Deno is that it treats TypeScript and JavaScript pretty
equally. This might mean that transitioning from JavaScript to TypeScript or
even from TypeScript to JavaScript is something you want to accomplish. There
are several features of Deno that can help with this.

### Type checking JavaScript

You might have some JavaScript that you would like to ensure is more type sound
but you don't want to go through a process of adding type annotations
everywhere.

Deno supports using the TypeScript type checker to type check JavaScript. You
can mark any individual file by adding the check JavaScript pragma to the file:

```js
// @ts-check
```

This will cause the type checker to infer type information about the JavaScript
code and raise any issues as diagnostic issues.

These can be turned on for all JavaScript files in a program by providing a
configuration file with the check JS option enabled:

```json
{
  "compilerOptions": {
    "checkJs": true
  }
}
```

And setting the `--config` option on the command line.

### Using JSDoc in JavaScript

If you are type checking JavaScript, or even importing JavaScript into
TypeScript you can use JSDoc in JavaScript to express more types information
than can just be inferred from the code itself. Deno supports this without any
additional configuration, you simply need to annotate the code in line with the
supported
[TypeScript JSDoc](https://www.typescriptlang.org/docs/handbook/jsdoc-supported-types.html).
For example to set the type of an array:

```js
/** @type {string[]} */
const a = [];
```

### Skipping type checking

You might have TypeScript code that you are experimenting with, where the syntax
is valid but not fully type safe. You can always bypass type checking for a
whole program by passing the `--no-check`.

You can also skip whole files being type checked, including JavaScript if you
have check JS enabled, by using the no-check pragma:

```js
// @ts-nocheck
```

### Just renaming JS files to TS files

While this might work in some cases, it has some severe limits in Deno. This is
because Deno, by default, runs type checking in what is called _strict mode_.
This means a lot of unclear or ambiguous situations where are not caught in
non-strict mode will result in diagnostics being generated, and JavaScript is
nothing but unclear and ambiguous when it comes to types.
