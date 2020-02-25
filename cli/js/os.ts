// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";
import { errors } from "./errors.ts";
import * as util from "./util.ts";

/** Check if running in terminal.
 *
 *       console.log(Deno.isTTY().stdout);
 */
export function isTTY(): { stdin: boolean; stdout: boolean; stderr: boolean } {
  return sendSync("op_is_tty");
}
/** Get the loadavg.
 * Requires the `--allow-env` flag.
 *
 *       console.log(Deno.loadavg());
 */
export function loadavg(): number[] {
  return sendSync("op_loadavg");
}

/** Get the hostname.
 * Requires the `--allow-env` flag.
 *
 *       console.log(Deno.hostname());
 */
export function hostname(): string {
  return sendSync("op_hostname");
}

/** Get OS release.
 * Requires the `--allow-env` flag.
 *
 *       console.log(Deno.osRelease());
 */
export function osRelease(): string {
  return sendSync("op_os_release");
}

/** Exit the Deno process with optional exit code. */
export function exit(code = 0): never {
  sendSync("op_exit", { code });
  return util.unreachable();
}

function setEnv(key: string, value: string): void {
  sendSync("op_set_env", { key, value });
}

function getEnv(key: string): string | undefined {
  return sendSync("op_get_env", { key })[0];
}

/** Returns a snapshot of the environment variables at invocation. Mutating a
 * property in the object will set that variable in the environment for
 * the process. The environment object will only accept `string`s
 * as values.
 *
 *       console.log(Deno.env("SHELL"));
 *       const myEnv = Deno.env();
 *       console.log(myEnv.SHELL);
 *       myEnv.TEST_VAR = "HELLO";
 *       const newEnv = Deno.env();
 *       console.log(myEnv.TEST_VAR == newEnv.TEST_VAR);
 */
export function env(): { [index: string]: string };
export function env(key: string): string | undefined;
export function env(
  key?: string
): { [index: string]: string } | string | undefined {
  if (key) {
    return getEnv(key);
  }
  const env = sendSync("op_env");
  return new Proxy(env, {
    set(obj, prop: string, value: string): boolean {
      setEnv(prop, value);
      return Reflect.set(obj, prop, value);
    }
  });
}

type DirKind =
  | "home"
  | "cache"
  | "config"
  | "executable"
  | "data"
  | "data_local"
  | "audio"
  | "desktop"
  | "document"
  | "download"
  | "font"
  | "picture"
  | "public"
  | "template"
  | "video";

/**
 * Returns the user and platform specific directories.
 * Requires the `--allow-env` flag.
 * Returns null if there is no applicable directory or if any other error
 * occurs.
 *
 * Argument values: "home", "cache", "config", "executable", "data",
 * "data_local", "audio", "desktop", "document", "download", "font", "picture",
 * "public", "template", "video"
 *
 * "cache"
 * |Platform | Value                               | Example                      |
 * | ------- | ----------------------------------- | ---------------------------- |
 * | Linux   | `$XDG_CACHE_HOME` or `$HOME`/.cache | /home/alice/.cache           |
 * | macOS   | `$HOME`/Library/Caches              | /Users/Alice/Library/Caches  |
 * | Windows | `{FOLDERID_LocalAppData}`           | C:\Users\Alice\AppData\Local |
 *
 * "config"
 * |Platform | Value                                 | Example                          |
 * | ------- | ------------------------------------- | -------------------------------- |
 * | Linux   | `$XDG_CONFIG_HOME` or `$HOME`/.config | /home/alice/.config              |
 * | macOS   | `$HOME`/Library/Preferences           | /Users/Alice/Library/Preferences |
 * | Windows | `{FOLDERID_RoamingAppData}`           | C:\Users\Alice\AppData\Roaming   |
 *
 * "executable"
 * |Platform | Value                                                           | Example                |
 * | ------- | --------------------------------------------------------------- | -----------------------|
 * | Linux   | `XDG_BIN_HOME` or `$XDG_DATA_HOME`/../bin or `$HOME`/.local/bin | /home/alice/.local/bin |
 * | macOS   | -                                                               | -                      |
 * | Windows | -                                                               | -                      |
 *
 * "data"
 * |Platform | Value                                    | Example                                  |
 * | ------- | ---------------------------------------- | ---------------------------------------- |
 * | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
 * | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
 * | Windows | `{FOLDERID_RoamingAppData}`              | C:\Users\Alice\AppData\Roaming           |
 *
 * "data_local"
 * |Platform | Value                                    | Example                                  |
 * | ------- | ---------------------------------------- | ---------------------------------------- |
 * | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
 * | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
 * | Windows | `{FOLDERID_LocalAppData}`                | C:\Users\Alice\AppData\Local             |
 *
 * "audio"
 * |Platform | Value              | Example              |
 * | ------- | ------------------ | -------------------- |
 * | Linux   | `XDG_MUSIC_DIR`    | /home/alice/Music    |
 * | macOS   | `$HOME`/Music      | /Users/Alice/Music   |
 * | Windows | `{FOLDERID_Music}` | C:\Users\Alice\Music |
 *
 * "desktop"
 * |Platform | Value                | Example                |
 * | ------- | -------------------- | ---------------------- |
 * | Linux   | `XDG_DESKTOP_DIR`    | /home/alice/Desktop    |
 * | macOS   | `$HOME`/Desktop      | /Users/Alice/Desktop   |
 * | Windows | `{FOLDERID_Desktop}` | C:\Users\Alice\Desktop |
 *
 * "document"
 * |Platform | Value                  | Example                  |
 * | ------- | ---------------------- | ------------------------ |
 * | Linux   | `XDG_DOCUMENTS_DIR`    | /home/alice/Documents    |
 * | macOS   | `$HOME`/Documents      | /Users/Alice/Documents   |
 * | Windows | `{FOLDERID_Documents}` | C:\Users\Alice\Documents |
 *
 * "download"
 * |Platform | Value                  | Example                  |
 * | ------- | ---------------------- | ------------------------ |
 * | Linux   | `XDG_DOWNLOAD_DIR`     | /home/alice/Downloads    |
 * | macOS   | `$HOME`/Downloads      | /Users/Alice/Downloads   |
 * | Windows | `{FOLDERID_Downloads}` | C:\Users\Alice\Downloads |
 *
 * "font"
 * |Platform | Value                                                | Example                        |
 * | ------- | ---------------------------------------------------- | ------------------------------ |
 * | Linux   | `$XDG_DATA_HOME`/fonts or `$HOME`/.local/share/fonts | /home/alice/.local/share/fonts |
 * | macOS   | `$HOME/Library/Fonts`                                | /Users/Alice/Library/Fonts     |
 * | Windows | –                                                    | –                              |
 *
 * "picture"
 * |Platform | Value                 | Example                 |
 * | ------- | --------------------- | ----------------------- |
 * | Linux   | `XDG_PICTURES_DIR`    | /home/alice/Pictures    |
 * | macOS   | `$HOME`/Pictures      | /Users/Alice/Pictures   |
 * | Windows | `{FOLDERID_Pictures}` | C:\Users\Alice\Pictures |
 *
 * "public"
 * |Platform | Value                 | Example             |
 * | ------- | --------------------- | ------------------- |
 * | Linux   | `XDG_PUBLICSHARE_DIR` | /home/alice/Public  |
 * | macOS   | `$HOME`/Public        | /Users/Alice/Public |
 * | Windows | `{FOLDERID_Public}`   | C:\Users\Public     |
 *
 * "template"
 * |Platform | Value                  | Example                                                    |
 * | ------- | ---------------------- | ---------------------------------------------------------- |
 * | Linux   | `XDG_TEMPLATES_DIR`    | /home/alice/Templates                                      |
 * | macOS   | –                      | –                                                          |
 * | Windows | `{FOLDERID_Templates}` | C:\Users\Alice\AppData\Roaming\Microsoft\Windows\Templates |
 *
 * "video"
 * |Platform | Value               | Example               |
 * | ------- | ------------------- | --------------------- |
 * | Linux   | `XDG_VIDEOS_DIR`    | /home/alice/Videos    |
 * | macOS   | `$HOME`/Movies      | /Users/Alice/Movies   |
 * | Windows | `{FOLDERID_Videos}` | C:\Users\Alice\Videos |
 */
export function dir(kind: DirKind): string | null {
  try {
    return sendSync("op_get_dir", { kind });
  } catch (error) {
    if (error instanceof errors.PermissionDenied) {
      throw error;
    }
    return null;
  }
}

/**
 * Returns the path to the current deno executable.
 * Requires the `--allow-env` flag.
 */
export function execPath(): string {
  return sendSync("op_exec_path");
}
