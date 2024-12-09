Disallows calling built-in global objects like functions

The following built-in objects should not be invoked like functions, even though
they look like constructors:

- `Math`
- `JSON`
- `Reflect`
- `Atomics`

Calling these as functions would result in runtime errors. This rule statically
prevents such wrong usage of them.

### Invalid:

```typescript
const math = Math();
const newMath = new Math();

const json = JSON();
const newJSON = new JSON();

const reflect = Reflect();
const newReflect = new Reflect();

const atomics = Atomics();
const newAtomics = new Atomics();
```

### Valid:

```typescript
const area = (radius: number): number => Math.PI * radius * radius;

const parsed = JSON.parse("{ foo: 42 }");

const x = Reflect.get({ x: 1, y: 2 }, "x");

const first = Atomics.load(foo, 0);
```
