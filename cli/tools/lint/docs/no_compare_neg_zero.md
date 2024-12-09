Disallows comparing against negative zero (`-0`).

Comparing a value directly against negative may not work as expected as it will
also pass for non-negative zero (i.e. `0` and `+0`). Explicit comparison with
negative zero can be performed using `Object.is`.

### Invalid:

```typescript
if (x === -0) {}
```

### Valid:

```typescript
if (x === 0) {}

if (Object.is(x, -0)) {}
```
