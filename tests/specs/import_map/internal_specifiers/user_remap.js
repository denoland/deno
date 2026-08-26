// A remapped `node:` specifier in *user* code must keep working.
import net from "node:net";

console.log("remapped:", net.remapped);
