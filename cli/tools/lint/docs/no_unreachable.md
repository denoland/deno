Disallows the unreachable code after the control flow statements.

Because the control flow statements (`return`, `throw`, `break` and `continue`)
unconditionally exit a block of code, any statements after them cannot be
executed.

### Invalid:

```typescript
function foo() {
  return true;
  console.log("done");
}
```

```typescript
function bar() {
  throw new Error("Oops!");
  console.log("done");
}
```

```typescript
while (value) {
  break;
  console.log("done");
}
```

```typescript
throw new Error("Oops!");
console.log("done");
```

```typescript
function baz() {
  if (Math.random() < 0.5) {
    return;
  } else {
    throw new Error();
  }
  console.log("done");
}
```

```typescript
for (;;) {}
console.log("done");
```

### Valid

```typescript
function foo() {
  return bar();
  function bar() {
    return 1;
  }
}
```
