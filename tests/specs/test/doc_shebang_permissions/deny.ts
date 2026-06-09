/**
 * `--deny-*` flags can't be represented in the `Deno.test` permissions object,
 * so they are dropped (with a warning) and only the allow is forwarded.
 *
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read --deny-read=/etc
 * Deno.cwd();
 * ```
 * @module doc
 */
