Disallows the deletion of variables

`delete` is used to remove a property from an object. Variables declared via
`var`, `let` and `const` cannot be deleted (`delete` will return `false`).
Setting `strict` mode on will raise a syntax error when attempting to delete a
variable.

### Invalid:

```typescript
const a = 1;
let b = 2;
let c = 3;
delete a; // would return false
delete b; // would return false
delete c; // would return false
```

### Valid:

```typescript
let obj = {
  a: 1,
};
delete obj.a; // return true
```
