# tracing

This is a tool for debugging the npm resolution.

To use it, compile with `--feature tracing`. For example:

```sh
cargo test grand_child_package_has_self_as_peer_dependency_root --features tracing -- --nocapture
```

This will output something like:

```
==============
Trace output ready! Please open your browser to: file:///.../deno-npm-trace.html
==============
```

Follow that and open your browser to see the output.
