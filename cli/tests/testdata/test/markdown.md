# Documentation

The following block does not have a language attribute and should be ignored:

```
This is a fenced block without attributes, it's invalid and it should be ignored.
```

The following examples should throw an error and should fail:

```js
throw new Error("Oh no, it broke!");
```

```jsx
throw new Error("Oh no, it broke!");
```

```ts
throw new Error("Oh no, it broke!");
```

```tsx
throw new Error("Oh no, it broke!");
```

The following examples would have thrown but the `no_run` attribute is there so
they should pass:

```js no_run
throw new Error("Oh no, js!");
```

```jsx no_run
throw new Error("Oh no, jsx!");
```

```ts no_run
throw new Error("Oh no, ts!");
```

```tsx no_run
throw new Error("Oh no, tsx broke!");
```

The following block should be given a js extension:

```js
console.assert(import.meta.url.endsWith(".js"));
```

The following block should be given a jsx extension:

```js
console.assert(import.meta.url.endsWith(".jsx"));
```

The following block should be given a ts extension:

```ts
console.assert(import.meta.url.endsWith(".ts"));
```

The following block should be given a ts extension:

```tsx
console.assert(import.meta.url.endsWith(".tsx"));
```

The following example has syntax errors but contains the ignore attribute and
should be ignored:

```ts ignore
const value: Invalid = "ignored";
```

The following example has type errors but should not trigger the type-checker
because of the `no_check` attribute:

```ts no_check
const a: string = 42;
```
