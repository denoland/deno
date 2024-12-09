Disallows the use of control flow statements within `finally` blocks.

Use of the control flow statements (`return`, `throw`, `break` and `continue`)
overrides the usage of any control flow statements that might have been used in
the `try` or `catch` blocks, which is usually not the desired behaviour.

### Invalid:

```typescript
let foo = function () {
  try {
    return 1;
  } catch (err) {
    return 2;
  } finally {
    return 3;
  }
};
```

```typescript
let foo = function () {
  try {
    return 1;
  } catch (err) {
    return 2;
  } finally {
    throw new Error();
  }
};
```

### Valid:

```typescript
let foo = function () {
  try {
    return 1;
  } catch (err) {
    return 2;
  } finally {
    console.log("hola!");
  }
};
```
