Requires all functions to have explicit return types.

Explicit return types have a number of advantages including easier to understand
code and better type safety. It is clear from the signature what the return type
of the function (if any) will be.

### Invalid:

```typescript
function someCalc() {
  return 2 * 2;
}
function anotherCalc() {
  return;
}
```

### Valid:

```typescript
function someCalc(): number {
  return 2 * 2;
}
function anotherCalc(): void {
  return;
}
```
