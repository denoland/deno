Disallows the usage of negation operator `!` as the left operand of relational
operators.

`!` operators appearing in the left operand of the following operators will
sometimes cause an unexpected behavior because of the operator precedence:

- `in` operator
- `instanceof` operator

For example, when developers write a code like `!key in someObject`, most likely
they want it to behave just like `!(key in someObject)`, but actually it behaves
like `(!key) in someObject`. This lint rule warns such usage of `!` operator so
it will be less confusing.

### Invalid:

<!-- deno-fmt-ignore -->

```typescript
if (!key in object) {}
if (!foo instanceof Foo) {}
```

### Valid:

```typescript
if (!(key in object)) {}
if (!(foo instanceof Foo)) {}
if ((!key) in object) {}
if ((!foo) instanceof Foo) {}
```
