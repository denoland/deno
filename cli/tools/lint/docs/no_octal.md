Disallows expressing octal numbers via numeric literals beginning with `0`

Octal numbers can be expressed via numeric literals with leading `0` like `042`,
but this expression often confuses programmers. That's why ECMAScript's strict
mode throws `SyntaxError` for the expression.

Since ES2015, the other prefix `0o` has been introduced as an alternative. This
new one is always encouraged to use in today's code.

### Invalid:

```typescript
const a = 042;
const b = 7 + 042;
```

### Valid:

```typescript
const a = 0o42;
const b = 7 + 0o42;
const c = "042";
```
