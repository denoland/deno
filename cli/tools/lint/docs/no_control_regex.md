Disallows the use ascii control characters in regular expressions

Control characters are invisible characters in the ASCII range of 0-31. It is
uncommon to use these in a regular expression and more often it is a mistake in
the regular expression.

### Invalid:

```typescript
// Examples using ASCII (31) Carriage Return (hex x0d)
const pattern1 = /\x0d/;
const pattern2 = /\u000d/;
const pattern3 = new RegExp("\\x0d");
const pattern4 = new RegExp("\\u000d");
```

### Valid:

```typescript
// Examples using ASCII (32) Space (hex x20)
const pattern1 = /\x20/;
const pattern2 = /\u0020/;
const pattern3 = new RegExp("\\x20");
const pattern4 = new RegExp("\\u0020");
```
