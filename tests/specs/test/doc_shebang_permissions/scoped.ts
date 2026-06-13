/**
 * A scoped `--deny-*` can't be represented in the `Deno.test` permissions
 * object, so the example fails instead of running with broader permissions
 * than the shebang declares.
 *
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read --deny-read=/etc
 * Deno.cwd();
 * ```
 * @module doc
 */
