Enforces the use of block scoped variables over more error prone function scoped
variables. Block scoped variables are defined using `const` and `let` keywords.

`const` and `let` keywords ensure the variables defined using these keywords are
not accessible outside their block scope. On the other hand, variables defined
using `var` keyword are only limited by their function scope.

### Invalid:

```typescript
var foo = "bar";
```

### Valid:

```typescript
const foo = 1;
let bar = 2;
```
