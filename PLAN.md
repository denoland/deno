# Plan: Add `--unix-socket` flag to `deno serve`

This plan outlines the changes required to allow `deno serve` to listen on a Unix domain socket instead of a TCP port.

## 1. Modify CLI Arguments (`cli/args/flags.rs`)

- **Update `ServeFlags` struct**: Add a `unix_socket: Option<String>` field.
- **Update `serve_subcommand()`**:
    - Add the `--unix-socket` argument using `clap`.
    - Ensure it conflicts with `--port` and `--host` (or overrides them).
    - Add a value hint for `FilePath`.
- **Update `serve_parse()`**:
    - Extract the `unix-socket` value from `ArgMatches` and populate the `ServeFlags` struct.

## 2. Update Server Logic (`cli/tools/serve.rs`)

- **Update `serve()` function**:
    - Handle the logic for when `unix_socket` is provided.
    - If `unix_socket` is set, the `resolve_serve_url` logic needs to be bypassed or adjusted to return a `unix://` or similar descriptive string for the `--open` flag and logging.
- **Pass the socket path to the runtime**:
    - The `deno serve` command works by looking for a default export that `Deno.serve` can use.
    - We need to ensure the internal configuration passed to the worker factory includes the Unix socket path so that the underlying `Deno.serve` implementation (likely in `ext/http`) knows to bind to a Unix socket instead of a TCP listener.
    - *Note*: `Deno.serve` already supports Unix sockets via the `path` option in JS. We need to bridge the CLI flag to that internal configuration.

## 3. Verification

- Build the binary: `cargo build --bin deno`.
- Test with a sample server: `./target/debug/deno serve --unix-socket /tmp/deno.sock server.ts`.
- Verify the socket file is created and accessible.
