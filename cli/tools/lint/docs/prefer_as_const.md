Recommends using const assertion (`as const`) over explicitly specifying literal
types or using type assertion.

When declaring a new variable of a primitive literal type, there are three ways:

1. adding an explicit type annotation
2. using normal type assertion (like `as "foo"`, or `<"foo">`)
3. using const assertion (`as const`)

This lint rule suggests using const assertion because it will generally lead to
a safer code. For more details about const assertion, see
[the official handbook](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-3-4.html#const-assertions).

### Invalid:

```typescript
let a: 2 = 2; // type annotation
let b = 2 as 2; // type assertion
let c = <2> 2; // type assertion
let d = { foo: 1 as 1 }; // type assertion
```

### Valid:

```typescript
let a = 2 as const;
let b = 2 as const;
let c = 2 as const;
let d = { foo: 1 as const };

let x = 2;
let y: string = "hello";
let z: number = someVariable;
```
