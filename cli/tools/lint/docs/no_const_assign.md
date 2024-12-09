Disallows modifying a variable declared as `const`.

Modifying a variable declared as `const` will result in a runtime error.

### Invalid:

```typescript
const a = 0;
a = 1;
a += 1;
a++;
++a;
```

### Valid:

```typescript
const a = 0;
const b = a + 1;

// `c` is out of scope on each loop iteration, allowing a new assignment
for (const c in [1, 2, 3]) {}
```
