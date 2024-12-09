Disallows self assignments

Self assignments like `a = a;` have no effect at all. If there are self
assignments in the code, most likely it means that the author is still in the
process of refactoring and there's remaining work they have to do.

### Invalid:

```typescript
a = a;
[a] = [a];
[a, b] = [a, b];
[a, b] = [a, c];
[a, ...b] = [a, ...b];
a.b = a.b;
```

### Valid:

```typescript
let a = a;
a += a;
a = [a];
[a, b] = [b, a];
a.b = a.c;
```
