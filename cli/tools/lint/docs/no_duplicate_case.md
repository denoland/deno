Disallows using the same case clause in a switch statement more than once

When you reuse a case test expression in a `switch` statement, the duplicate
case will never be reached meaning this is almost always a bug.

### Invalid:

```typescript
const someText = "a";
switch (someText) {
  case "a": // (1)
    break;
  case "b":
    break;
  case "a": // duplicate of (1)
    break;
  default:
    break;
}
```

### Valid:

```typescript
const someText = "a";
switch (someText) {
  case "a":
    break;
  case "b":
    break;
  case "c":
    break;
  default:
    break;
}
```
