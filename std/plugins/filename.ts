// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
export function pluginFilename(
  filenameBase: string,
  os?: Deno.OperatingSystem
): string {
  os = os || Deno.build.os;

  let prefix = "";
  let extension = "";

  if (os === "linux") {
    prefix = "lib";
    extension = "so";
  } else if (os === "mac") {
    prefix = "lib";
    extension = "dylib";
  } else if (os === "win") {
    extension = "dll";
  } else {
    throw TypeError("Bad `os` value.");
  }

  return `${prefix}${filenameBase}.${extension}`;
}
