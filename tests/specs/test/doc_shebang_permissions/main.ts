/**
 * The shebang only requests read access, so the inherited `--allow-env`
 * permission is revoked and accessing the environment fails.
 *
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read
 * Deno.env.toObject();
 * ```
 *
 * `-A` inherits every permission from the test runner, so this example can
 * access the environment.
 *
 * ```ts
 * #!/usr/bin/env -S deno run -A
 * Deno.env.toObject();
 * ```
 * @module doc
 */
