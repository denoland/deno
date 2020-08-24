## Linter

Deno ships with a built in code linter for JavaScript and TypeScript.

**Note: linter is a new feature and still unstable thus it requires `--unstable`
flag**

```shell
# lint all JS/TS files in the current directory and subdirectories
deno lint --unstable
# lint specific files
deno lint --unstable myfile1.ts myfile2.ts
```

### Available rules

- `ban-ts-comment`
- `ban-untagged-ignore`
- `constructor-super`
- `for-direction`
- `getter-return`
- `no-array-constructor`
- `no-async-promise-executor`
- `no-case-declarations`
- `no-class-assign`
- `no-compare-neg-zero`
- `no-cond-assign`
- `no-debugger`
- `no-delete-var`
- `no-dupe-args`
- `no-dupe-keys`
- `no-duplicate-case`
- `no-empty-character-class`
- `no-empty-interface`
- `no-empty-pattern`
- `no-empty`
- `no-ex-assign`
- `no-explicit-any`
- `no-func-assign`
- `no-misused-new`
- `no-namespace`
- `no-new-symbol`
- `no-obj-call`
- `no-octal`
- `no-prototype-builtins`
- `no-regex-spaces`
- `no-setter-return`
- `no-this-alias`
- `no-this-before-super`
- `no-unsafe-finally`
- `no-unsafe-negation`
- `no-with`
- `prefer-as-const`
- `prefer-namespace-keyword`
- `require-yield`
- `triple-slash-reference`
- `use-isnan`
- `valid-typeof`

### Ignore directives

#### Files

To ignore whole file `// deno-lint-ignore-file` directive should placed at the
top of the file:

```ts
// deno-lint-ignore-file

function foo(): any {
  // ...
}
```

Ignore directive must be placed before first stament or declaration:

```ts
// Copyright 2020 the Deno authors. All rights reserved. MIT license.

/**
 * Some JS doc
 **/

// deno-lint-ignore-file

import { bar } from "./bar.js";

function foo(): any {
  // ...
}
```

#### Diagnostics

To ignore certain diagnostic `// deno-lint-ignore <codes...>` directive should
be placed before offending line. Specifying ignored rule name is required:

```ts
// deno-lint-ignore no-explicit-any
function foo(): any {
  // ...
}

// deno-lint-ignore no-explicit-any explicit-function-return-type
function bar(a: any) {
  // ...
}
```

To provide some compatibility with ESLint `deno lint` also supports
`// eslint-ignore-next-line` directive. Just like with `// deno-lint-ignore`,
it's required to specify the ignored rule name:

```ts
// eslint-ignore-next-line no-empty
while (true) {}

// eslint-ignore-next-line @typescript-eslint/no-explicit-any
function bar(a: any) {
  // ...
}
```
