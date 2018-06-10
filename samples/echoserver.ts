import { NetSocket, Socket } from "deno";
import { NetServerConn, createServer } from "deno";

const decoder = new TextDecoder("utf-8");

const server = createServer((conn: NetServerConn) => {
    conn.onData((rawData: Uint8Array) => {
        const data = decoder.decode(rawData);
        if (data === "quit") {
            conn.close();
        }
        conn.write(data);
        conn.write("\n");
    });
});
server.listen(5001);

setTimeout(() => {}, 50000);
