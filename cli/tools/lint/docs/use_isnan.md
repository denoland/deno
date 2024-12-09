Disallows comparisons to `NaN`.

Because `NaN` is unique in JavaScript by not being equal to anything, including
itself, the results of comparisons to `NaN` are confusing:

- `NaN === NaN` or `NaN == NaN` evaluate to `false`
- `NaN !== NaN` or `NaN != NaN` evaluate to `true`

Therefore, this rule makes you use the `isNaN()` or `Number.isNaN()` to judge
the value is `NaN` or not.

### Invalid:

```typescript
if (foo == NaN) {
  // ...
}

if (foo != NaN) {
  // ...
}

switch (NaN) {
  case foo:
    // ...
}

switch (foo) {
  case NaN:
    // ...
}
```

### Valid:

```typescript
if (isNaN(foo)) {
  // ...
}

if (!isNaN(foo)) {
  // ...
}
```
