Suggests using frozen intrinsics from `primordials` rather than the default
globals.

This lint rule is designed to be dedicated to Deno's internal code. Normal users
don't have to run this rule for their code.

Primordials are a frozen set of all intrinsic objects in the runtime, which we
should use in the Deno's internal to avoid the risk of prototype pollution. This
rule detects the direct use of global intrinsics and suggests replacing it with
the corresponding one from the `primordials` object.

One such example is:

```javascript
const arr = getSomeArrayOfNumbers();
const evens = arr.filter((val) => val % 2 === 0);
```

The second line of this example should be:

```javascript
const evens = primordials.ArrayPrototypeFilter(arr, (val) => val % 2 === 0);
```

### Invalid:

```javascript
const arr = new Array();

const s = JSON.stringify({});

const i = parseInt("42");

const { ownKeys } = Reflect;
```

### Valid:

```javascript
const { Array } = primordials;
const arr = new Array();

const { JSONStringify } = primordials;
const s = JSONStringify({});

const { NumberParseInt } = primordials;
const i = NumberParseInt("42");

const { ReflectOwnKeys } = primordials;
```
