Disallows redeclaration of variables, functions, parameters with the same name.

JavaScript allows us to redeclare variables with the same name using `var`, but
redeclaration should not be used since it can make variables hard to trace.

In addition, this lint rule disallows redeclaration using `let` or `const` as
well, although ESLint allows. This is useful because we can notice a syntax
error before actually running the code.

As for functions and parameters, JavaScript just treats these as runtime errors,
throwing `SyntaxError` when being run. It's also beneficial to detect this sort
of errors statically.

### Invalid:

```typescript
var a = 3;
var a = 10;

let b = 3;
let b = 10;

const c = 3;
const c = 10;

function d() {}
function d() {}

function e(arg: number) {
  var arg: number;
}

function f(arg: number, arg: string) {}
```

### Valid:

```typescript
var a = 3;
function f() {
  var a = 10;
}

if (foo) {
  let b = 2;
} else {
  let b = 3;
}
```
