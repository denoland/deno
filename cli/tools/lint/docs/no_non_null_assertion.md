Disallow non-null assertions using the `!` postfix operator

TypeScript's `!` non-null assertion operator asserts to the type system that an
expression is non-nullable, as in not `null` or `undefined`. Using assertions to
tell the type system new information is often a sign that code is not fully
type-safe. It's generally better to structure program logic so that TypeScript
understands when values may be nullable.

### Invalid:

```typescript
interface Example {
  property?: string;
}
declare const example: Example;

const includes = example.property!.includes("foo");
```

### Valid:

```typescript
interface Example {
  property?: string;
}
declare const example: Example;

const includes = example.property?.includes("foo") ?? false;
```
