Requires `deno-lint-ignore` to be annotated with one or more rule names.

Ignoring all rules can mask unexpected or future problems. Therefore you need to
explicitly specify which rule(s) are to be ignored.

### Invalid:

```typescript
// deno-lint-ignore
export function duplicateArgumentsFn(a, b, a) {}
```

### Valid:

```typescript
// deno-lint-ignore no-dupe-args
export function duplicateArgumentsFn(a, b, a) {}
```
