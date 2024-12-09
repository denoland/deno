Disallows using the same condition twice in an `if`/`else if` statement

When you reuse a condition in an `if`/`else if` statement, the duplicate
condition will never be reached (without unusual side-effects) meaning this is
almost always a bug.

### Invalid:

```typescript
if (a) {}
else if (b) {}
else if (a) {} // duplicate of condition above

if (a === 5) {}
else if (a === 6) {}
else if (a === 5) {} // duplicate of condition above
```

### Valid:

```typescript
if (a) {}
else if (b) {}
else if (c) {}

if (a === 5) {}
else if (a === 6) {}
else if (a === 7) {}
```
