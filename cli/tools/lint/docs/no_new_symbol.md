Disallows the use of `new` operators with built-in `Symbol`s

`Symbol`s are created by being called as a function, but we sometimes call it
with the `new` operator by mistake. This rule detects such wrong usage of the
`new` operator.

### Invalid:

```typescript
const foo = new Symbol("foo");
```

### Valid:

```typescript
const foo = Symbol("foo");

function func(Symbol: typeof SomeClass) {
  // This `Symbol` is not built-in one
  const bar = new Symbol();
}
```
