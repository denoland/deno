# Documentation

The following block does not have a language attribute and should be ignored:

```
This is a fenced block without attributes, it's invalid and it should be ignored.
```

The following block should be given a javascript extension on extraction:

```javascript
console.log("javascript");
```

The following block should be given a typescript extension on extraction:

```typescript
console.log("typescript");
```

The following example contains the ignore attribute and will be ignored:

```typescript ignore
const value: Invalid = "ignored";
```

The following example will trigger the type-checker to fail:

```typescript
const a: string = 42;
```
