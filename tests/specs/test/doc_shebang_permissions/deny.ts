/**
 * `--deny-env` revokes env, but every other permission is still inherited from
 * the test runner, so reading the cwd keeps working.
 *
 * ```ts
 * #!/usr/bin/env -S deno run --deny-env
 * Deno.cwd();
 * ```
 *
 * The same shebang denies env, so accessing the environment fails.
 *
 * ```ts
 * #!/usr/bin/env -S deno run --deny-env
 * Deno.env.toObject();
 * ```
 * @module doc
 */
