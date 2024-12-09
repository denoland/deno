Disallows specifying invalid regular expressions in RegExp constructors

Specifying an invalid regular expression literal will result in a SyntaxError at
compile time, however specifying an invalid regular expression string in the
RegExp constructor will only be discovered at runtime.

### Invalid:

```typescript
const invalidRegExp = new RegExp(")");
```

### Valid:

```typescript
const goodRegExp = new RegExp(".");
```
