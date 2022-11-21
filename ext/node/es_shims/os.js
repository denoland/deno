const m = Deno[Deno.internal].nodePolyfills.os;

export const arch = m.arch;
export const cpus = m.cpus;
export const endianness = m.endianness;
export const freemem = m.freemem;
export const getPriority = m.getPriority;
export const homedir = m.homedir;
export const hostname = m.hostname;
export const loadavg = m.loadavg;
export const networkInterfaces = m.networkInterfaces;
export const platform = m.platform;
export const setPriority = m.setPriority;
export const tmpdir = m.tmpdir;
export const totalmem = m.totalmem;
export const type = m.type;
export const uptime = m.uptime;
export const userInfo = m.userInfo;
export const constants = m.constants;
export const EOL = m.EOL;
export const devNull = m.devNull;

export default m;
