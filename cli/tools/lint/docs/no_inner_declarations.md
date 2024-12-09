Disallows variable or function definitions in nested blocks

Function declarations in nested blocks can lead to less readable code and
potentially unexpected results due to compatibility issues in different
JavaScript runtimes. This does not apply to named or anonymous functions which
are valid in a nested block context.

Variables declared with `var` in nested blocks can also lead to less readable
code. Because these variables are hoisted to the module root, it is best to
declare them there for clarity. Note that variables declared with `let` or
`const` are block scoped and therefore this rule does not apply to them.

### Invalid:

```typescript
if (someBool) {
  function doSomething() {}
}

function someFunc(someVal: number): void {
  if (someVal > 4) {
    var a = 10;
  }
}
```

### Valid:

```typescript
function doSomething() {}
if (someBool) {}

var a = 10;
function someFunc(someVal: number): void {
  var foo = true;
  if (someVal > 4) {
    let b = 10;
    const fn = function doSomethingElse() {};
  }
}
```
