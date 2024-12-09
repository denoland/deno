Disallows use of the `any` type

Use of the `any` type disables the type check system around that variable,
defeating the purpose of Typescript which is to provide type safe code.
Additionally, the use of `any` hinders code readability, since it is not
immediately clear what type of value is being referenced. It is better to be
explicit about all types. For a more type-safe alternative to `any`, use
`unknown` if you are unable to choose a more specific type.

### Invalid:

```typescript
const someNumber: any = "two";
function foo(): any {
  return undefined;
}
```

### Valid:

```typescript
const someNumber: string = "two";
function foo(): undefined {
  return undefined;
}
```
