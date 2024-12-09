Disallows duplicate keys in object literals.

Setting the same key multiple times in an object literal will override other
assignments to that key and can cause unexpected behaviour.

### Invalid:

```typescript
const foo = {
  bar: "baz",
  bar: "qux",
};
```

```typescript
const foo = {
  "bar": "baz",
  bar: "qux",
};
```

```typescript
const foo = {
  0x1: "baz",
  1: "qux",
};
```

### Valid:

```typescript
const foo = {
  bar: "baz",
  quxx: "qux",
};
```
