Disallows the declaration of an empty interface

An interface with no members serves no purpose. This rule will capture these
situations as either unnecessary code or a mistaken empty implementation.

### Invalid:

```typescript
interface Foo {}
```

### Valid:

```typescript
interface Foo {
  name: string;
}

interface Bar {
  age: number;
}

// Using an empty interface with at least one extension are allowed.

// Using an empty interface to change the identity of Baz from type to interface.
type Baz = { profession: string };
interface Foo extends Baz {}

// Using an empty interface to extend already existing Foo declaration
// with members of the Bar interface
interface Foo extends Bar {}

// Using an empty interface as a union type
interface Baz extends Foo, Bar {}
```
