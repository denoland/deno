## Web Storage API

As of Deno 1.10, the Web Storage API (`localStorage` & `sessionStorage`) was
introduced, which through `localStorage` allows persistent storage, whereas
`sessionStorage` is a non-persistent memory-based storage.

To use persistent storage, you need to pass the `--location` flag. The location
for persistent storage is listed in `deno info`, and additionally passing the
`--location` will give you the path for the specified origin.

### Example

The following snippet accesses the local storage bucket for the current origin
and adds a data item to it using `setItem()`.

```ts
localStorage.setItem("myDemo", "Deno App");
```

The syntax for reading the localStorage item is as follows:

```ts
const cat = localStorage.getItem("myDemo");
```

The syntax for removing the localStorage item is as follows:

```ts
localStorage.removeItem("myDemo");
```

The syntax for removing all the localStorage items is as follows:

```ts
localStorage.clear();
```
