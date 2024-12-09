Disallows unnecessary non-null assertions

Non-null assertions are specified with an `!` saying to the compiler that you
know this value is not null. Specifying this operator more than once in a row,
or in combination with the optional chaining operator (`?`) is confusing and
unnecessary.

### Invalid:

```typescript
const foo: { str: string } | null = null;
const bar = foo!!.str;

function myFunc(bar: undefined | string) {
  return bar!!;
}
function anotherFunc(bar?: { str: string }) {
  return bar!?.str;
}
```

### Valid:

```typescript
const foo: { str: string } | null = null;
const bar = foo!.str;

function myFunc(bar: undefined | string) {
  return bar!;
}
function anotherFunc(bar?: { str: string }) {
  return bar?.str;
}
```
