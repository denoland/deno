Disallows defining `constructor`s for interfaces or `new` for classes

Specifying a `constructor` for an interface or defining a `new` method for a
class is incorrect and should be avoided.

### Invalid:

```typescript
class C {
  new(): C;
}

interface I {
  constructor(): void;
}
```

### Valid:

```typescript
class C {
  constructor() {}
}

interface I {
  new (): C;
}
```
