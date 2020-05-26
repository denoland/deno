## Documentation Generator

`deno doc` followed by a list of source files will print the JSDoc documentation
for each module's **exported** members.

```ts
/**
 * Returns the sum of x and y.
 * @param {number} x
 * @param {number} y
 * @returns {number} The sum of x and y
 */
export function sum (x: number, y: number): number {
  return x + y;
}
```

```shell
deno doc https://deno.land/std/fs/copy.ts
```

<!-- TODO(mattd3v): write more things, and add code examples -->
