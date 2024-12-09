Disallow non-null assertions after an optional chain expression

`?.` optional chain expressions provide undefined if an object is `null` or
`undefined`. Using a `!` non-null assertion to assert the result of an `?.`
optional chain expression is non-nullable is likely wrong.

### Invalid:

```typescript
foo?.bar!;
foo?.bar()!;
```

### Valid:

```typescript
foo?.bar;
foo?.bar();
```
