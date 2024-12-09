Bans the use of primitive wrapper objects (e.g. `String` the object is a wrapper
of `string` the primitive) in addition to the non-explicit `Function` type and
the misunderstood `Object` type.

There are very few situations where primitive wrapper objects are desired and
far more often a mistake was made with the case of the primitive type. You also
cannot assign a primitive wrapper object to a primitive leading to type issues
down the line. For reference, [the TypeScript handbook] also says we shouldn't
ever use these wrapper objects.

[the TypeScript handbook]: https://www.typescriptlang.org/docs/handbook/declaration-files/do-s-and-don-ts.html#number-string-boolean-symbol-and-object

With `Function`, it is better to explicitly define the entire function signature
rather than use the non-specific `Function` type which won't give you type
safety with the function.

Finally, `Object` and `{}` means "any non-nullish value" rather than "any object
type". `object` is a good choice for a meaning of "any object type".

### Invalid:

```typescript
let a: Boolean;
let b: String;
let c: Number;
let d: Symbol;
let e: Function;
let f: Object;
let g: {};
```

### Valid:

```typescript
let a: boolean;
let b: string;
let c: number;
let d: symbol;
let e: () => number;
let f: object;
let g: Record<string, never>;
```
