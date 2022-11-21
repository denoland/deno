const m = Deno[Deno.internal].nodePolyfills.process;

export const nextTick = m.nextTick;
export const arch = m.arch;
export const chdir = m.chdir;
export const cwd = m.cwd;
export const env = m.env;
export const pid = m.pid;
export const platform = m.platform;
export const version = m.version;
export const versions = m.versions;
export const stdin = m.stdin;
export const stdout = m.stdout;
export const stderr = m.stderr;
export const exit = m.exit;
export const emitWarning = m.emitWarning;
export const kill = m.kill;
export const removeListener = m.removeListener;
export const removeAllListeners = m.removeAllListeners;
export const process = m.process;

export default m;
