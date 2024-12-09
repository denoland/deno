Requires `await` is not used in a for loop body

Async and await are used in Javascript to provide parallel execution. If each
element in the for loop is waited upon using `await`, then this negates the
benefits of using async/await as no more elements in the loop can be processed
until the current element finishes.

A common solution is to refactor the code to run the loop body asynchronously
and capture the promises generated. After the loop finishes you can then await
all the promises at once.

### Invalid:

```javascript
async function doSomething(items) {
  const results = [];
  for (const item of items) {
    // Each item in the array blocks on the previous one finishing
    results.push(await someAsyncProcessing(item));
  }
  return processResults(results);
}
```

### Valid:

```javascript
async function doSomething(items) {
  const results = [];
  for (const item of items) {
    // Kick off all item processing asynchronously...
    results.push(someAsyncProcessing(item));
  }
  // ...and then await their completion after the loop
  return processResults(await Promise.all(results));
}
```
