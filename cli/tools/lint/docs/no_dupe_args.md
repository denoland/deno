Disallows using an argument name more than once in a function signature

If you supply multiple arguments of the same name to a function, the last
instance will shadow the preceding one(s). This is most likely an unintentional
typo.

### Invalid:

```typescript
function withDupes(a, b, a) {
  console.log("I'm the value of the second a:", a);
}
```

### Valid:

```typescript
function withoutDupes(a, b, c) {
  console.log("I'm the value of the first (and only) a:", a);
}
```
