Disallows generator functions that have no `yield`.

JavaScript provides generator functions expressed as `function*`, where we can
pause and later resume the function execution at the middle points. At these
points we use the `yield` keyword. In other words, it makes no sense at all to
create generator functions that contain no `yield` keyword, since such functions
could be written as normal functions.

### Invalid:

```typescript
function* f1() {
  return "f1";
}
```

### Valid:

```typescript
function* f1() {
  yield "f1";
}

// generator function with empty body is allowed
function* f2() {}

function f3() {
  return "f3";
}
```
