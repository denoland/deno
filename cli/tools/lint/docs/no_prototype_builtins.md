Disallows the use of `Object.prototype` builtins directly

If objects are created via `Object.create(null)` they have no prototype
specified. This can lead to runtime errors when you assume objects have
properties from `Object.prototype` and attempt to call the following methods:

- `hasOwnProperty`
- `isPrototypeOf`
- `propertyIsEnumerable`

Instead, it's always encouraged to call these methods from `Object.prototype`
explicitly.

### Invalid:

```typescript
const a = foo.hasOwnProperty("bar");
const b = foo.isPrototypeOf("bar");
const c = foo.propertyIsEnumerable("bar");
```

### Valid:

```typescript
const a = Object.prototype.hasOwnProperty.call(foo, "bar");
const b = Object.prototype.isPrototypeOf.call(foo, "bar");
const c = Object.prototype.propertyIsEnumerable.call(foo, "bar");
```
