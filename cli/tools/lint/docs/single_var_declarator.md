Disallows multiple variable definitions in the same declaration statement

### Invalid:

```typescript
const foo = 1, bar = "2";
```

### Valid:

```typescript
const foo = 1;
const bar = "2";
```
