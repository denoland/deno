Disallows the use of empty patterns in destructuring

In destructuring, it is possible to use empty patterns such as `{}` or `[]`
which have no effect, most likely not what the author intended.

### Invalid:

```typescript
// In these examples below, {} and [] are not object literals or empty arrays,
// but placeholders for destructured variable names
const {} = someObj;
const [] = someArray;
const {a: {}} = someObj;
const [a: []] = someArray;
function myFunc({}) {}
function myFunc([]) {}
```

### Valid:

```typescript
const { a } = someObj;
const [a] = someArray;

// Correct way to default destructured variable to object literal
const { a = {} } = someObj;

// Correct way to default destructured variable to empty array
const [a = []] = someArray;

function myFunc({ a }) {}
function myFunc({ a = {} }) {}
function myFunc([a]) {}
function myFunc([a = []]) {}
```
