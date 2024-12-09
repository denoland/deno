Disallows the overwriting/reassignment of an existing function

Javascript allows for the reassignment of a function definition. This is
generally a mistake on the developers part, or poor coding practice as code
readability and maintainability will suffer.

### Invalid:

```typescript
function foo() {}
foo = bar;

const a = function baz() {
  baz = "now I'm a string";
};

myFunc = existingFunc;
function myFunc() {}
```

### Valid:

```typescript
function foo() {}
const someVar = foo;

const a = function baz() {
  const someStr = "now I'm a string";
};

const anotherFuncRef = existingFunc;

let myFuncVar = function () {};
myFuncVar = bar; // variable reassignment, not function re-declaration
```
