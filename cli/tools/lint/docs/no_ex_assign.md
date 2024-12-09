Disallows the reassignment of exception parameters

There is generally no good reason to reassign an exception parameter. Once
reassigned the code from that point on has no reference to the error anymore.

### Invalid:

```typescript
try {
  someFunc();
} catch (e) {
  e = true;
  // can no longer access the thrown error
}
```

### Valid:

```typescript
try {
  someFunc();
} catch (e) {
  const anotherVar = true;
}
```
