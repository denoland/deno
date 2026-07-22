/**
 * A shebang that is not a valid `deno` command fails the test rather than
 * running with inherited permissions.
 *
 * ```ts
 * #!/bin/sh
 * Deno.env.toObject();
 * ```
 * @module doc
 */
