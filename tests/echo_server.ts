import { args, listen, copy } from "deno";
const addr = args[1] || "127.0.0.1:4544";
const listener = listen("tcp", addr);
console.log("listening on", addr);
listener.accept().then(async conn => {
  await copy(conn, conn);
  conn.close();
  listener.close();
});
