# Plan: Add `--unix-socket` flag to `deno serve`

This plan outlines the changes required to allow `deno serve` to listen on a Unix domain socket instead of a TCP port.

## 1. Modify CLI Arguments (`cli/args/flags.rs`)

- **Update `ServeFlags` struct**: Add a `unix_socket: Option<String>` field.
- **Update `serve_subcommand()`**:
    - Add the `--unix-socket` argument using `clap`.
    - Ensure it conflicts with `--port` and `--host`.
    - Add a value hint for `FilePath`.
- **Update `serve_parse()`**:
    - Extract the `unix-socket` value from `ArgMatches` and populate the `ServeFlags` struct.

## 2. Update Server Logic (`cli/tools/serve.rs`)

- **Update `serve()` function**:
    - If `unix_socket` is set, disable the `--open` functionality as browsers cannot open Unix sockets directly.
    - Set the `DENO_SERVE_ADDRESS` environment variable to `unix:<path>` before creating the worker.
    - *Discovery*: The `deno serve` command relies on the runtime's internal JS bootstrap to automatically call `Deno.serve` on the default export. The runtime looks at the `DENO_SERVE_ADDRESS` environment variable (already defined in `ENV_VARS` in `flags.rs`) to override the default TCP listener.
- **Environment Variable Hand-off**:
    - By setting `DENO_SERVE_ADDRESS` in the Rust process before the worker starts, the internal JS `Deno.serve` call will pick up the Unix socket path automatically.

## 3. Verification

- Build the binary: `cargo build --bin deno`.
- Test with a sample server: `./target/debug/deno serve --unix-socket /tmp/deno.sock server.ts`.
- Verify the socket file is created: `ls -l /tmp/deno.sock`.
- Test connectivity: `curl --unix-socket /tmp/deno.sock http://localhost/`.
