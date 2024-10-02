# Documentation

The following block does not have a language attribute and should be ignored:

```
This is a fenced block without attributes, it's invalid and it should be ignored.
```

The following block should be given a js extension on extraction:

```js
console.log("js");
```

The following block should be given a ts extension on extraction:

```ts
console.log("ts");
```

The following example contains the ignore attribute and will be ignored:

```ts ignore
const value: Invalid = "ignored";
```

The following example will trigger the type-checker to fail:

```ts
const a: string = 42;
```
