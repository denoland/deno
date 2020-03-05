// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { PermissionDenied } = Deno.errors;

/** Attempts to grant a given permission, resolving with `true` if it is
 * granted or `false` if it isn't.
 *
 *      if (grant({ name: "env" })) {
 *        // ...
 *      } else {
 *        console.error("Cannot access the environment.");
 *      }
 *
 * If the permission is denied, the function will resolve with `false`.  If the
 * permission is not granted, but can be prompted for, the function will prompt
 * for it. If the permission is granted, the function resolves with `true`. */
export async function grant(
  descriptor: Deno.PermissionDescriptor
): Promise<boolean> {
  let state = (await Deno.permissions.query(descriptor)).state;
  if (state === "prompt") {
    state = (await Deno.permissions.request(descriptor)).state;
  }
  return state === "granted" ? true : false;
}

/** Attempts to grant a given permission or throws.
 *
 *      await grantOrThrow({ name: "env" });
 *
 * If the permission is denied, this function will throw with a
 * `Deno.errors.PermissionDenied`.  If the permission is not granted, but can be
 * prompted for, this function will prompt for it.  If the permission is
 * granted, the function resolves.*/
export async function grantOrThrow(
  descriptor: Deno.PermissionDescriptor
): Promise<void> {
  let state = (await Deno.permissions.query(descriptor)).state;
  if (state === "prompt") {
    state = (await Deno.permissions.request(descriptor)).state;
  }
  if (state !== "granted") {
    throw new PermissionDenied(
      `Permission for "${descriptor.name}" has been denied.`
    );
  }
}
