## Web Storage API

As of Deno 1.10, the Web Storage API (`localStorage` & `sessionStorage`) was
introduced, which through `localStorage` allows persistent storage, whereas
`sessionStorage` is a non-persistent memory-based storage.

To use persistent storage, you need to pass the `--location` flag. The location
for persistent storage is listed in `deno info`, and additionally passing the
`--location` will give you the path for the specified origin.
