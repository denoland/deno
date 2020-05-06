## Code formatter

Deno ships with a built in code formatter that auto-formats TypeScript and
JavaScript code.

```shell
# format all JS/TS files in the current directory and subdirectories
deno fmt
# format specific files
deno fmt myfile1.ts myfile2.ts
# check if all the JS/TS files in the current directory and subdirectories are formatted
deno fmt --check
# format stdin and write to stdout
cat file.ts | deno fmt -
```

Ignore formatting code by preceding it with a `// deno-fmt-ignore` comment:

<!-- prettier-ignore-start -->

```ts
// deno-fmt-ignore
export const identity = [
    1, 0, 0,
    0, 1, 0,
    0, 0, 1,
];
```

<!-- prettier-ignore-end -->

Or ignore an entire file by adding a `// deno-fmt-ignore-file` comment at the
top of the file.
