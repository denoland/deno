// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import process from "./_process.ts";

/** https://nodejs.org/api/process.html#process_process_arch */
export const arch = process.arch;

/** https://nodejs.org/api/process.html#process_process_argv */
export const argv = process.argv;

/** https://nodejs.org/api/process.html#process_process_chdir_directory */
export const chdir = process.chdir;

/** https://nodejs.org/api/process.html#process_process_cwd */
export const cwd = process.cwd;

/** https://nodejs.org/api/process.html#process_process_env */
export const env = process.env;

/** https://nodejs.org/api/process.html#process_process_exit_code */
export const exit = process.exit;

/** https://nodejs.org/api/process.html#process_process_nexttick_callback_args */
export const nextTick = process.nextTick;

/** https://nodejs.org/api/process.html#process_process_pid */
export const pid = process.pid;

/** https://nodejs.org/api/process.html#process_process_platform */
export const platform = process.platform;

/** https://nodejs.org/api/process.html#process_process_stderr */
export const stderr = process.stderr;

/** https://nodejs.org/api/process.html#process_process_stdin */
export const stdin = process.stdin;

/** https://nodejs.org/api/process.html#process_process_stdout */
export const stdout = process.stdout;

/** https://nodejs.org/api/process.html#process_process_version */
export const version = process.version;

/** https://nodejs.org/api/process.html#process_process_versions */
export const versions = process.versions;

export default process;

//TODO
//Remove on 1.0
//Kept for backwars compatibility with std
export { process };
