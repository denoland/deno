Requires lexical declarations (`let`, `const`, `function` and `class`) in switch
`case` or `default` clauses to be scoped with brackets.

Without brackets in the `case` or `default` block, the lexical declarations are
visible to the entire switch block but only get initialized when they are
assigned, which only happens if that case/default is reached. This can lead to
unexpected errors. The solution is to ensure each `case` or `default` block is
wrapped in brackets to scope limit the declarations.

### Invalid:

```typescript
switch (choice) {
  // `let`, `const`, `function` and `class` are scoped the entire switch statement here
  case 1:
    let a = "choice 1";
    break;
  case 2:
    const b = "choice 2";
    break;
  case 3:
    function f() {
      return "choice 3";
    }
    break;
  default:
    class C {}
}
```

### Valid:

```typescript
switch (choice) {
  // The following `case` and `default` clauses are wrapped into blocks using brackets
  case 1: {
    let a = "choice 1";
    break;
  }
  case 2: {
    const b = "choice 2";
    break;
  }
  case 3: {
    function f() {
      return "choice 3";
    }
    break;
  }
  default: {
    class C {}
  }
}
```
