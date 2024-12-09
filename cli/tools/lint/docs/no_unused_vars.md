Enforces all variables are used at least once.

If there are variables that are declared but not used anywhere, it's most likely
because of incomplete refactoring. This lint rule detects and warns such unused
variables.

Variable `a` is considered to be "used" if any of the following conditions are
satisfied:

- its value is read out, like `console.log(a)` or `let otherVariable = a;`
- it's called or constructed, like `a()` or `new a()`
- it's exported, like `export const a = 42;`

If a variable is just assigned to a value but never read out, then it's
considered to be _"not used"_.

```typescript
let a;
a = 42;

// `a` is never read out
```

If you want to declare unused variables intentionally, prefix them with the
underscore character `_`, like `_a`. This rule ignores variables that are
prefixed with `_`.

### Invalid:

```typescript
const a = 0;

const b = 0; // this `b` is never used
function foo() {
  const b = 1; // this `b` is used
  console.log(b);
}
foo();

let c = 2;
c = 3;

// recursive function calls are not considered to be used, because only when `d`
// is called from outside the function body can we say that `d` is actually
// called after all.
function d() {
  d();
}

// `x` is never used
export function e(x: number): number {
  return 42;
}

const f = "unused variable";
```

### Valid:

```typescript
const a = 0;
console.log(a);

const b = 0;
function foo() {
  const b = 1;
  console.log(b);
}
foo();
console.log(b);

let c = 2;
c = 3;
console.log(c);

function d() {
  d();
}
d();

export function e(x: number): number {
  return x + 42;
}

export const f = "exported variable";
```
