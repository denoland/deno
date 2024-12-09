Disallows use of `this` or `super` before calling `super()` in constructors.

The access to `this` or `super` before calling `super()` in the constructor of
derived classes leads to [`ReferenceError`]. To prevent it, this lint rule
checks if there are accesses to `this` or `super` before calling `super()` in
constructors.

[`ReferenceError`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/ReferenceError

### Invalid:

```typescript
class A extends B {
  constructor() {
    this.foo = 0;
    super();
  }
}

class C extends D {
  constructor() {
    super.foo();
    super();
  }
}
```

### Valid:

```typescript
class A extends B {
  constructor() {
    super();
    this.foo = 0;
  }
}

class C extends D {
  constructor() {
    super();
    super.foo();
  }
}

class E {
  constructor() {
    this.foo = 0;
  }
}
```
