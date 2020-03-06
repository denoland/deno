// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { PermissionDenied } = Deno.errors;

function getPermissionString(descriptors: Deno.PermissionDescriptor[]): string {
  return descriptors
    .map(pd => {
      switch (pd.name) {
        case "read":
        case "write":
          return pd.path ? `${pd.name}(${pd.path})` : pd.name;
        case "net":
          return pd.url ? `${pd.name}(${pd.url})` : pd.url;
        default:
          return pd.name;
      }
    })
    .join(", ");
}

/** Attempts to grant a set of permissions, resolving with the descriptors of
 * the permissions that are granted.
 *
 *      const perms = await grant({ name: "net" }, { name: "read" });
 *      if (perms && perms.length === 2) {
 *        // do something cool that connects to the net and reads files
 *      } else {
 *        // notify user of missing permissions
 *      }
 *
 * If one of the permissions requires a prompt, the function will attempt to
 * prompt for it.  The function resolves with all of the granted permissions. */
export async function grant(
  ...descriptors: Deno.PermissionDescriptor[]
): Promise<void | Deno.PermissionDescriptor[]>;
/** Attempts to grant a set of permissions, resolving with the descriptors of
 * the permissions that are granted.
 *
 *      const perms = await grant([{ name: "net" }, { name: "read" }]);
 *      if (perms && perms.length === 2) {
 *        // do something cool that connects to the net and reads files
 *      } else {
 *        // notify user of missing permissions
 *      }
 *
 * If one of the permissions requires a prompt, the function will attempt to
 * prompt for it.  The function resolves with all of the granted permissions. */
export async function grant(
  descriptors: Deno.PermissionDescriptor[]
): Promise<void | Deno.PermissionDescriptor[]>;
export async function grant(
  descriptor: Deno.PermissionDescriptor[] | Deno.PermissionDescriptor,
  ...descriptors: Deno.PermissionDescriptor[]
): Promise<void | Deno.PermissionDescriptor[]> {
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
  return result.length ? result : undefined;
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
 * If any of the permissions are denied, the function will reject mentioning the
 * the denied permissions.  If all permissions are granted, the function will
 * resolve. */
export async function grantOrThrow(
  descriptors: Deno.PermissionDescriptor[]
): Promise<void>;
export async function grantOrThrow(
  descriptor: Deno.PermissionDescriptor[] | Deno.PermissionDescriptor,
  ...descriptors: Deno.PermissionDescriptor[]
): Promise<void> {
  const denied: Deno.PermissionDescriptor[] = [];
  descriptors = Array.isArray(descriptor)
    ? descriptor
    : [descriptor, ...descriptors];
  for (const descriptor of descriptors) {
    let state = (await Deno.permissions.query(descriptor)).state;
    if (state === "prompt") {
      state = (await Deno.permissions.request(descriptor)).state;
    }
    if (state !== "granted") {
      denied.push(descriptor);
    }
  }
  if (denied.length) {
    throw new PermissionDenied(
      `The following permissions are denied: ${getPermissionString(denied)}`
    );
  }
}
