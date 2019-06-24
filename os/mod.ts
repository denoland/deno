// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/**
 * Returns the current user's home directory.
 * On Unix, including macOS, it returns the $HOME environment variable.
 * On Windows, it returns %USERPROFILE%.
 * Needs permissions to access env (--allow-env).
 *
 * Ported from Go: https://github.com/golang/go/blob/go1.12.5/src/os/file.go#L389
 */
export function userHomeDir(): string {
  let env = "HOME";
  let envErr = "$HOME";

  if (Deno.platform.os === "win") {
    env = "USERPROFILE";
    envErr = "%USERPROFILE%";
  }

  const value = Deno.env()[env];
  if (value !== "") {
    return value;
  }

  throw new Error(`Environment variable '${envErr}' is not defined.`);
}
