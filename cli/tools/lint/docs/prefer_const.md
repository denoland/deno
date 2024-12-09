Recommends declaring variables with [`const`] over [`let`].

Since ES2015, JavaScript supports [`let`] and [`const`] for declaring variables.
If variables are declared with [`let`], then they become mutable; we can set
other values to them afterwards. Meanwhile, if declared with [`const`], they are
immutable; we cannot perform re-assignment to them.

In general, to make the codebase more robust, maintainable, and readable, it is
highly recommended to use [`const`] instead of [`let`] wherever possible. The
fewer mutable variables are, the easier it should be to keep track of the
variable states while reading through the code, and thus it is less likely to
write buggy code. So this lint rule checks if there are [`let`] variables that
could potentially be declared with [`const`] instead.

Note that this rule does not check for [`var`] variables. Instead,
[the `no-var` rule](https://lint.deno.land/rules/no-var) is responsible for
detecting and warning [`var`] variables.

[`let`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/let
[`const`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/const
[`var`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/var

### Invalid:

```typescript
let a = 0;

let b = 0;
someOperation(b);

// `const` could be used instead
for (let c in someObject) {}

// `const` could be used instead
for (let d of someArray) {}

// variable that is uninitialized at first and then assigned in the same scope is NOT allowed
// because we could simply write it like `const e = 2;` instead
let e;
e = 2;
```

### Valid:

```typescript
// uninitialized variable is allowed
let a;

let b = 0;
b += 1;

let c = 0;
c = 1;

// variable that is uninitialized at first and then assigned in the same scope _two or more times_ is allowed
// because we cannot represent it with `const`
let d;
d = 2;
d = 3;

const e = 0;

// `f` is mutated through `f++`
for (let f = 0; f < someArray.length; f++) {}

// variable that is initialized (or assigned) in another scope is allowed
let g;
function func1() {
  g = 42;
}

// conditionally initialized variable is allowed
let h;
if (trueOrFalse) {
  h = 0;
}
```
