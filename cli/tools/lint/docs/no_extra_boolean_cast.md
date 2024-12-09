Disallows unnecessary boolean casts

In certain contexts, such as `if`, `while` or `for` statements, expressions are
automatically coerced into a boolean. Therefore, techniques such as double
negation (`!!foo`) or casting (`Boolean(foo)`) are unnecessary and produce the
same result as without the negation or casting.

### Invalid:

```typescript
if (!!foo) {}
if (Boolean(foo)) {}
while (!!foo) {}
for (; Boolean(foo);) {}
```

### Valid:

```typescript
if (foo) {}
while (foo) {}
for (; foo;) {}
```
