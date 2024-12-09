Requires that async promise executor functions are not used

Promise constructors take an executor function as an argument with `resolve` and
`reject` parameters that can be used to control the state of the created
Promise. This function is allowed to be async but this is generally not a good
idea for several reasons:

- If an async executor function throws an error, the error will be lost and
  won't cause the newly-constructed Promise to reject. This could make it
  difficult to debug and handle some errors.
- If an async Promise executor function is using await, then this is usually a
  sign that it is not actually necessary to use the new Promise constructor and
  the code can be restructured to avoid the use of a promise, or the scope of
  the new Promise constructor can be reduced, extracting the async code and
  changing it to be synchronous.

### Invalid:

```typescript
new Promise(async function (resolve, reject) {});
new Promise(async (resolve, reject) => {});
```

### Valid:

```typescript
new Promise(function (resolve, reject) {});
new Promise((resolve, reject) => {});
```
