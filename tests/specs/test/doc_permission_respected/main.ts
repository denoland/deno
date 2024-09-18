/**
 * This should succeed because we pass `--allow-env=PATH`
 * ```ts
 * const _path = Deno.env.get("PATH");
 * ```
 *
 * This should fail because we don't allow for env access to `USER`
 * ```ts
 * const _user = Deno.env.get("USER");
 * ```
 * @module doc
 */
