// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { PermissionDenied } = Deno.errors;

/** Attempts to grant a set of permissions, resolving with the descriptors of
 * the permissions that are granted.
 *
 *      const perms = await grant({ name: "net" }, { name: "read" });
 *      if (perms.length === 2) {
 *        // do something cool that connects to the net and reads files
 *      } else {
 *        // notify user of missing permissions
 *      }
 *
 * If one of the permissions requires a prompt, the function will attempt to
 * prompt for it.  The function resolves with all of the granted permissions. */
export async function grant(
  ...descriptors: Deno.PermissionDescriptor[]
): Promise<Deno.PermissionDescriptor[]>;
/** Attempts to grant a set of permissions, resolving with the descriptors of
 * the permissions that are granted.
 *
 *      const perms = await grant([{ name: "net" }, { name: "read" }]);
 *      if (perms.length === 2) {
 *        // do something cool that connects to the net and reads files
 *      } else {
 *        // notify user of missing permissions
 *      }
 *
 * If one of the permissions requires a prompt, the function will attempt to
 * prompt for it.  The function resolves with all of the granted permissions. */
export async function grant(
  descriptors: Deno.PermissionDescriptor[]
): Promise<Deno.PermissionDescriptor[]>;
export async function grant(
  descriptor: Deno.PermissionDescriptor[] | Deno.PermissionDescriptor,
  ...descriptors: Deno.PermissionDescriptor[]
): Promise<Deno.PermissionDescriptor[]> {
  const result: Deno.PermissionDescriptor[] = [];
  descriptors = Array.isArray(descriptor)
    ? descriptor
    : [descriptor, ...descriptors];
  for (const descriptor of descriptors) {
    let state = (await Deno.permissions.query(descriptor)).state;
    if (state === "prompt") {
      state = (await Deno.permissions.request(descriptor)).state;
    }
    if (state === "granted") {
      result.push(descriptor);
    }
  }
  return result;
}

/** Attempts to grant a set of permissions or rejects.
 *
 *      await grantOrThrow({ name: "env" }, { name: "net" });
 *
 * If the permission can be prompted for, the function will attempt to prompt.
 * If any of the permissions are denied, the function will reject for the first
 * permission that is denied.  If all permissions are granted, the function
 * will resolve. */
export async function grantOrThrow(
  ...descriptors: Deno.PermissionDescriptor[]
): Promise<void>;
/** Attempts to grant a set of permissions or rejects.
 *
 *      await grantOrThrow([{ name: "env" }, { name: "net" }]);
 *
 * If the permission can be prompted for, the function will attempt to prompt.
 * If any of the permissions are denied, the function will reject for the first
 * permission that is denied.  If all permissions are granted, the function
 * will resolve. */
export async function grantOrThrow(
  descriptors: Deno.PermissionDescriptor[]
): Promise<void>;
export async function grantOrThrow(
  descriptor: Deno.PermissionDescriptor[] | Deno.PermissionDescriptor,
  ...descriptors: Deno.PermissionDescriptor[]
): Promise<void> {
  descriptors = Array.isArray(descriptor)
    ? descriptor
    : [descriptor, ...descriptors];
  for (const descriptor of descriptors) {
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
}

/** Attempts to grant the requested permissions, resolving with `true` if it is
 * granted or `false` if it isn't.
 *
 *      if (await isGranted({ name: "env" })) {
 *        // ...
 *      } else {
 *        console.error("Cannot access the environment.");
 *      }
 *
 * If the permission is denied, the function will resolve with `false`.  If the
 * permission is not granted, but can be prompted for, the function will prompt
 * for it. If the permission is granted, the function resolves with `true`. */
export async function isGranted(
  descriptor: Deno.PermissionDescriptor
): Promise<boolean> {
  let state = (await Deno.permissions.query(descriptor)).state;
  if (state === "prompt") {
    state = (await Deno.permissions.request(descriptor)).state;
  }
  return state === "granted" ? true : false;
}
