// `node:http`'s polyfill internally imports `node:net`, which the import map
// remaps. Internal modules must not go through the user's import map.
import http from "node:http";

console.log("createServer:", typeof http.createServer);
