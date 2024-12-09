Requires all property getter functions to return a value

Getter functions return the value of a property. If the function returns no
value then this contract is broken.

### Invalid:

```typescript
let foo = {
  get bar() {},
};

class Person {
  get name() {}
}
```

### Valid:

```typescript
let foo = {
  get bar() {
    return true;
  },
};

class Person {
  get name() {
    return "alice";
  }
}
```
