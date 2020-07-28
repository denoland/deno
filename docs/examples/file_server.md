## File server

This one serves a local directory in HTTP.

```shell
deno install --allow-net --allow-read https://deno.land/std/http/file_server.ts
```

Run it:

```shell
$ file_server .
Downloading https://deno.land/std/http/file_server.ts...
[...]
HTTP server listening on http://0.0.0.0:4500/
```

And if you ever want to upgrade to the latest published version:

```shell
file_server --reload
```
