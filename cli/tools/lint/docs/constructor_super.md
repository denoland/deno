Verifies the correct usage of constructors and calls to `super()`.

Defined constructors of derived classes (e.g. `class A extends B`) must always
call `super()`. Classes which extend non-constructors (e.g.
`class A extends null`) must not have a constructor.

### Invalid:

```typescript
class A {}
class Z {
  constructor() {}
}

class B extends Z {
  constructor() {} // missing super() call
}
class C {
  constructor() {
    super(); // Syntax error
  }
}
class D extends null {
  constructor() {} // illegal constructor
}
class E extends null {
  constructor() { // illegal constructor
    super();
  }
}
```

### Valid:

```typescript
class A {}
class B extends A {}
class C extends A {
  constructor() {
    super();
  }
}
class D extends null {}
```
