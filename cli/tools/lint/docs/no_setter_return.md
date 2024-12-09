Disallows returning values from setters.

Setters are supposed to be used for setting some value to the property, which
means that returning a value from a setter makes no sense. In fact, returned
values are ignored and cannot ever be used at all although returning a value
from a setter produces no error. This is why static check for this mistake by
the linter is quite beneficial.

Note that returning without a value is allowed; this is a useful technique to do
early-return from a function.

### Invalid:

```typescript
const a = {
  set foo(x: number) {
    return "something";
  },
};

class B {
  private set foo(x: number) {
    return "something";
  }
}

const c = {
  set foo(x: boolean) {
    if (x) {
      return 42;
    }
  },
};
```

### Valid:

```typescript
// return without a value is allowed since it is used to do early-return
const a = {
  set foo(x: number) {
    if (x % 2 == 0) {
      return;
    }
  },
};

// not a setter, but a getter
class B {
  get foo() {
    return 42;
  }
}

// not a setter
const c = {
  set(x: number) {
    return "something";
  },
};
```
