Disallows the declaration of an empty enum

An enum with no members serves no purpose. This rule will capture these
situations as either unnecessary code or a mistaken empty implementation.

### Invalid:

```typescript
enum Foo {}
```

### Valid:

```typescript
enum Foo {
  ONE = "ONE",
}
```
