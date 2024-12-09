Disallows the use of the assignment operator, `=`, in conditional statements.

Use of the assignment operator within a conditional statement is often the
result of mistyping the equality operator, `==`. If an assignment within a
conditional statement is required then this rule allows it by wrapping the
assignment in parentheses.

### Invalid:

```typescript
let x;
if (x = 0) {
  let b = 1;
}
```

```typescript
function setHeight(someNode) {
  do {
    someNode.height = "100px";
  } while (someNode = someNode.parentNode);
}
```

### Valid:

```typescript
let x;
if (x === 0) {
  let b = 1;
}
```

```typescript
function setHeight(someNode) {
  do {
    someNode.height = "100px";
  } while ((someNode = someNode.parentNode));
}
```
