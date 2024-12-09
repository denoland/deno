Warns the usage of unknown rule codes in ignore directives

We sometimes have to suppress and ignore lint errors for some reasons. We can do
so using [ignore directives](https://lint.deno.land/ignoring-rules) with rule
names that should be ignored like so:

```typescript
// deno-lint-ignore no-explicit-any no-unused-vars
const foo: any = 42;
```

This rule checks for the validity of the specified rule names (i.e. whether
`deno_lint` provides the rule or not).

### Invalid:

```typescript
// typo
// deno-lint-ignore eq-eq-e
console.assert(x == 42);

// unknown rule name
// deno-lint-ignore UNKNOWN_RULE_NAME
const b = "b";
```

### Valid:

```typescript
// deno-lint-ignore eq-eq-eq
console.assert(x == 42);

// deno-lint-ignore no-unused-vars
const b = "b";
```
