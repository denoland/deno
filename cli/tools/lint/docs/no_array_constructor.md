Enforce conventional usage of array construction

Array construction is conventionally done via literal notation such as `[]` or
`[1, 2, 3]`. Using the `new Array()` is discouraged as is `new Array(1, 2, 3)`.
There are two reasons for this. The first is that a single supplied argument
defines the array length, while multiple arguments instead populate the array of
no fixed size. This confusion is avoided when pre-populated arrays are only
created using literal notation. The second argument to avoiding the `Array`
constructor is that the `Array` global may be redefined.

The one exception to this rule is when creating a new array of fixed size, e.g.
`new Array(6)`. This is the conventional way to create arrays of fixed length.

### Invalid:

```typescript
// This is 4 elements, not a size 100 array of 3 elements
const a = new Array(100, 1, 2, 3);

const b = new Array(); // use [] instead
```

### Valid:

```typescript
const a = new Array(100);
const b = [];
const c = [1, 2, 3];
```
