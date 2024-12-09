Disallows assigning variables to `this`.

In most cases, storing a reference to `this` in a variable could be avoided by
using arrow functions properly, since they establish `this` based on the scope
where the arrow function is defined.

Let's take a look at a concrete example:

```typescript
const obj = {
  count: 0,
  doSomethingLater() {
    setTimeout(function () { // this function executes on the global scope; `this` evalutes to `globalThis`
      this.count++;
      console.log(this.count);
    }, 300);
  },
};

obj.doSomethingLater();
// `NaN` is printed, because the property `count` is not in the global scope.
```

In the above example, `this` in the function passed to `setTimeout` evaluates to
`globalThis`, which results in the expected value `1` not being printed.

If you wanted to work around it without arrow functions, you would store a
reference to `this` in another variable:

```typescript
const obj = {
  count: 0,
  doSomethingLater() {
    const self = this; // store a reference to `this` in `self`
    setTimeout(function () {
      // use `self` instead of `this`
      self.count++;
      console.log(self.count);
    }, 300);
  },
};

obj.doSomethingLater();
// `1` is printed as expected
```

But in this case arrow functions come in handy. With arrow functions, the code
becomes way clearer and easier to understand:

```typescript
const obj = {
  count: 0,
  doSomethingLater() {
    setTimeout(() => { // pass an arrow function
      // `this` evaluates to `obj` here
      this.count++;
      console.log(this.count);
    }, 300);
  },
};

obj.doSomethingLater();
// `1` is printed as expected
```

This example is taken from
[MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Functions/Arrow_functions).

### Invalid:

```typescript
const self = this;

function foo() {
  const self = this;
}

const bar = () => {
  const self = this;
};
```

### Valid:

```typescript
const self = "this";

const [foo] = this;
```
