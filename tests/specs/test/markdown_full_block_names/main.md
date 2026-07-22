# Documentation

The following block should be given a js extension on extraction:

```javascript
console.log("js");
```

The following example contains the ignore attribute and will be ignored:

```typescript ignore
const value: Invalid = "ignored";
```

The following example will trigger the type-checker to fail:

```typescript
const a: string = 42;
```
