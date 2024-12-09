Disallows sparse arrays

Sparse arrays are arrays that contain _empty slots_, which later could be
handled either as `undefined` value or skipped by array methods, and this may
lead to unexpected behavior:

```typescript
[1, , 2].join(); // => '1,,2'
[1, undefined, 2].join(); // => '1,,2'

[1, , 2].flatMap((item) => item); // => [1, 2]
[1, undefined, 2].flatMap((item) => item); // => [1, undefined, 2]
```

### Invalid:

```typescript
const items = ["foo", , "bar"];
```

### Valid:

```typescript
const items = ["foo", "bar"];
```
