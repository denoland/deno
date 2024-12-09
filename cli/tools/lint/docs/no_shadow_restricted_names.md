Disallows shadowing of restricted names.

The following (a) properties of the global object, or (b) identifiers are
"restricted" names in JavaScript:

- [`NaN`]
- [`Infinity`]
- [`undefined`]
- [`eval`]
- [`arguments`]

These names are _NOT_ reserved in JavaScript, which means that nothing prevents
one from assigning other values into them (i.e. shadowing). In other words, you
are allowed to use, say, `undefined` as an identifier or variable name. (For
more details see [MDN])

[`NaN`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/NaN
[`Infinity`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Infinity
[`undefined`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/undefined
[`eval`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/eval
[`arguments`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Functions/arguments
[MDN]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/undefined#description

```typescript
function foo() {
  const undefined = "bar";
  console.log(undefined); // output: "bar"
}
```

Of course, shadowing like this most likely confuse other developers and should
be avoided. This lint rule detects and warn them.

### Invalid:

```typescript
const undefined = 42;

function NaN() {}

function foo(Infinity) {}

const arguments = () => {};

try {
} catch (eval) {}
```

### Valid:

```typescript
// If not assigned a value, `undefined` may be shadowed
const undefined;

const Object = 42;

function foo(a: number, b: string) {}

try {
} catch (e) {}
```
