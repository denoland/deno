# guards

A comprehensive collection of type guards.

## Table of contents

- [Usage](#usage)

  - [Primitives](#primitives)

    - [`isBigInt`](#isbigint)
    - [`isBoolean`](#isboolean)
    - [`isNumber`](#isnumber)
    - [`isString`](#isstring)
    - [`isSymbol`](#issymbol)
    - [`isUndefined`](#isundefined)

  - [Special](#special)

    - [`isNull`](#isnull)
    - [`isFunction`](#isfunction)
    - [`isObject`](#isobject)
    - [`isArray`](#isarray)
    - [`isMap`](#ismap)
    - [`isSet`](#isset)
    - [`isWeakMap`](#isweakmap)
    - [`isWeakSet`](#isweakset)
    - [`isDate`](#isdate)

  - [Convenience](#convenience)

    - [`isNonEmptyArray`](#isnonemptyarray)
    - [`isValidNumber`](#isvalidnumber)
    - [`isInteger`](#isinteger)
    - [`isPositiveInteger`](#ispositiveinteger)
    - [`isNonNegativeInteger`](#isnonnegativeinteger)
    - [`isNegativeInteger`](#isnegativeinteger)

## Usage

From <https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures>:

The latest ECMAScript standard defines nine types:

- Six Data Types that are primitives, checked by `typeof` operator:
  - `undefined`: `typeof instance === "undefined"`
  - `Boolean`: `typeof instance === "boolean"`
  - `Number`: `typeof instance === "number"`
  - `String`: `typeof instance === "string"`
  - `BigInt`: `typeof instance === "bigint"`
  - `Symbol`: `typeof instance === "symbol"`
- `null`: `typeof instance === "object"`. Special primitive type having
  additional usage for it's value: if object is not inherited, then `null` is
  shown;
- `Object`: `typeof instance === "object"`. Special non-data but structural type
  for any constructed object instance also used as data structures: new
  `Object`, new `Array`, new `Map`, new `Set`, new `WeakMap`, new `WeakSet`, new
  `Date` and almost everything made with `new` keyword;
- `Function` non data structure, though it also answers for `typeof` operator:
  `typeof instance === "function"`. This answer is done as a special shorthand
  for `Function`s, though every `Function` constructor is derived from `Object`
  constructor.

### Primitives

#### `isBigInt`

```typescript
let val: bigint | number;

if (isBigint(val)) {
  // TypeScript will infer val: bigint
} else {
  // TypeScript will infer val: number
}
```

#### `isBoolean`

```typescript
let val: boolean | number;

if (isBoolean(val)) {
  // TypeScript will infer val: boolean
} else {
  // TypeScript will infer val: number
}
```

#### `isNumber`

```typescript
let val: number | string;

if (isNumber(val)) {
  // TypeScript will infer val: number
} else {
  // TypeScript will infer val: string
}
```

#### `isString`

```typescript
let val: string | number;

if (isString(val)) {
  // TypeScript will infer val: string
} else {
  // TypeScript will infer val: number
}
```

#### `isSymbol`

```typescript
let val: symbol | string;

if (isSymbol(val)) {
  // TypeScript will infer val: symbol
} else {
  // TypeScript will infer val: string
}
```

#### `isUndefined`

```typescript
let val: undefined | null;

if (isUndefined(val)) {
  // TypeScript will infer val: undefined
} else {
  // TypeScript will infer val: null
}
```

### Special

#### `isNull`

Answers `true` if and only if `value === null`.

Full TypeScript (type inference) support.

#### `isFunction`

Answers `true` if and only if `typeof value === "function"`.

Full TypeScript (type inference) support.

#### `isObject`

Answers `true` if and only if:

- `isNull(value) === false`; and
- `typeof value === "object"`

Full TypeScript (type inference) support.

#### `isArray`

Answers `true` if and only if `Array.isArray(value) === true`.

Full TypeScript (type inference) support.

#### `isMap`

Answers `true` if and only if `(value instanceof Map) === true`.

Full TypeScript (type inference) support.

#### `isSet`

Answers `true` if and only if `(value instanceof Set) === true`.

Full TypeScript (type inference) support.

#### `isWeakMap`

Answers `true` if and only if `(value instanceof WeakMap) === true`.

Full TypeScript (type inference) support.

#### `isWeakSet`

Answers `true` if and only if `(value instanceof WeakSet) === true`.

Full TypeScript (type inference) support.

#### `isDate`

Answers `true` if and only if `(value instanceof Date) === true`.

Full TypeScript (type inference) support.

### Convenience

#### `isNonEmptyArray`

```typescript
test("isNonEmptyArray", (): void => {
  assertEquals(convenience.isNonEmptyArray([1, 2]), true);
  assertEquals(convenience.isNonEmptyArray([1]), true);
  assertEquals(convenience.isNonEmptyArray([]), false);
});
```

Full TypeScript (type inference) support.

#### `isValidNumber`

```typescript
test("isValidNumber", (): void => {
  assertEquals(convenience.isValidNumber(0), true);
  assertEquals(convenience.isValidNumber(42), true);
  assertEquals(convenience.isValidNumber(-42), true);
  assertEquals(convenience.isValidNumber(3.14), true);
  assertEquals(convenience.isValidNumber(-3.14), true);
  assertEquals(convenience.isValidNumber(Infinity), true);
  assertEquals(convenience.isValidNumber(-Infinity), true);
  assertEquals(convenience.isValidNumber(Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isValidNumber(-Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isValidNumber(NaN), false);
});
```

Full TypeScript (type inference) support.

#### `isInteger`

```typescript
test("isInteger", (): void => {
  assertEquals(convenience.isInteger(0), true);
  assertEquals(convenience.isInteger(42), true);
  assertEquals(convenience.isInteger(-42), true);
  assertEquals(convenience.isInteger(3.14), false);
  assertEquals(convenience.isInteger(-3.14), false);
  assertEquals(convenience.isInteger(Infinity), false);
  assertEquals(convenience.isInteger(-Infinity), false);
  assertEquals(convenience.isInteger(Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isInteger(-Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isInteger(NaN), false);
});
```

Full TypeScript (type inference) support.

#### `isPositiveInteger`

```typescript
test("isPositiveInteger", (): void => {
  assertEquals(convenience.isPositiveInteger(0), false);
  assertEquals(convenience.isPositiveInteger(42), true);
  assertEquals(convenience.isPositiveInteger(-42), false);
  assertEquals(convenience.isPositiveInteger(3.14), false);
  assertEquals(convenience.isPositiveInteger(-3.14), false);
  assertEquals(convenience.isPositiveInteger(Infinity), false);
  assertEquals(convenience.isPositiveInteger(-Infinity), false);
  assertEquals(convenience.isPositiveInteger(Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isPositiveInteger(-Number.MAX_SAFE_INTEGER), false);
  assertEquals(convenience.isPositiveInteger(NaN), false);
});
```

Full TypeScript (type inference) support.

#### `isNonNegativeInteger`

```typescript
test("isNonNegativeInteger", (): void => {
  assertEquals(convenience.isNonNegativeInteger(0), true);
  assertEquals(convenience.isNonNegativeInteger(42), true);
  assertEquals(convenience.isNonNegativeInteger(-42), false);
  assertEquals(convenience.isNonNegativeInteger(3.14), false);
  assertEquals(convenience.isNonNegativeInteger(-3.14), false);
  assertEquals(convenience.isNonNegativeInteger(Infinity), false);
  assertEquals(convenience.isNonNegativeInteger(-Infinity), false);
  assertEquals(convenience.isNonNegativeInteger(Number.MAX_SAFE_INTEGER), true);
  assertEquals(
    convenience.isNonNegativeInteger(-Number.MAX_SAFE_INTEGER),
    false
  );
  assertEquals(convenience.isNonNegativeInteger(NaN), false);
});
```

Full TypeScript (type inference) support.

#### `isNegativeInteger`

```typescript
test("isNegativeInteger", (): void => {
  assertEquals(convenience.isNegativeInteger(0), false);
  assertEquals(convenience.isNegativeInteger(42), false);
  assertEquals(convenience.isNegativeInteger(-42), true);
  assertEquals(convenience.isNegativeInteger(3.14), false);
  assertEquals(convenience.isNegativeInteger(-3.14), false);
  assertEquals(convenience.isNegativeInteger(Infinity), false);
  assertEquals(convenience.isNegativeInteger(-Infinity), false);
  assertEquals(convenience.isNegativeInteger(Number.MAX_SAFE_INTEGER), false);
  assertEquals(convenience.isNegativeInteger(-Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isNegativeInteger(NaN), false);
});
```

Full TypeScript (type inference) support.
