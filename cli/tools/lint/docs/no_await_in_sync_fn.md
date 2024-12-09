Disallow `await` keyword inside a non-async function

Using the `await` keyword inside a non-async function is a syntax error. To be
able to use `await` inside a function, the function needs to be marked as async
via the `async` keyword

### Invalid:

```javascript
function foo() {
  await bar();
}

const fooFn = function foo() {
  await bar();
};

const fooFn = () => {
  await bar();
};
```

### Valid:

```javascript
async function foo() {
  await bar();
}

const fooFn = async function foo() {
  await bar();
};

const fooFn = async () => {
  await bar();
};
```
