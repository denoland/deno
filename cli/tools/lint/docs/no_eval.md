Disallows the use of `eval`

`eval` is a potentially dangerous function which can open your code to a number
of security vulnerabilities. In addition to being slow, `eval` is also often
unnecessary with better solutions available.

### Invalid:

```typescript
const obj = { x: "foo" };
const key = "x",
const value = eval("obj." + key);
```

### Valid:

```typescript
const obj = { x: "foo" };
const value = obj[x];
```
