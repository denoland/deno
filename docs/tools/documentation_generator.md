## Documentation Generator

`deno doc` followed by a list of source files will print the JSDoc documentation
for each of the module's **exported** members.

For example, given a file `add.ts` with the contents:

```ts
/**
 * Adds x and y.
 * @param {number} x
 * @param {number} y
 * @returns {number} Sum of x and y
 */
export function add (x: number, y: number): number {
  return x + y;
}
```

Executing the Deno CLI documentation command:

```shell
deno doc add.ts
```

Prints the following to `stdout`:

```
export function add (x: number, y: number): number 
  Adds x and y. @param {number} x @param {number} y @returns {number} Sum of x and y
```

<!-- TODO(mattd3v): write more things, and add code examples -->
