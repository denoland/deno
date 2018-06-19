import { pubInternal, sub } from "./dispatch";
import { main as pb } from "./msg.pb";

const sockets = new Map<number, NetSocket>();
const servers = new Map<number, NetServer>();
const clients = new Map<number, NetServerConn>();

const enc = new TextEncoder();

type ConnCb = (conn: NetServerConn) => void;
type DataCb = (data: Uint8Array) => void;

export function initNet() {
    sub("net", (payload: Uint8Array) => {
        const msg = pb.Msg.decode(payload);
        switch (msg.command) {
            case pb.Msg.Command.NET_SOCKET_CONNECT_OK: {
                const s = sockets.get(msg.netSocketId);
                s.onMsg(msg);
                break;
            }
            case pb.Msg.Command.NET_SOCKET_READ: {
                const s = sockets.get(msg.netSocketId);
                s.onMsg(msg);
                break;
            }
            case pb.Msg.Command.NET_SERVER_CLIENT_CONN: {
                const s = servers.get(msg.netServerClientConnServerId);
                s.onMsg(msg);
                break;
            }
            case pb.Msg.Command.NET_SERVER_CLIENT_READ: {
                const s = clients.get(msg.netServerClientId);
                s.onMsg(msg);
                break;
            }
            default: {
            }
        }
    });
  }

let nextSocketId = 0;
export class NetSocket {
    private readonly id: number;
    private connectCb: () => void;
    private onDataCb: DataCb;
    constructor() {
        this.id = nextSocketId++;
        sockets.set(this.id, this);
    }
    connect(port: number, address: string, cb: () => void) {
        this.connectCb = cb;
        pubInternal("net", {
            command: pb.Msg.Command.NET_SOCKET_CONNECT,
            netSocketAddr: address,
            netSocketPort: port
        });
    }
    write(data: Uint8Array | string) {
        if (typeof(data) === "string") {
            data = enc.encode(data);   
        }
        pubInternal("net",{
            command: pb.Msg.Command.NET_SOCKET_WRITE,
            netSocketId: this.id,
            netSocketData: data
        });
    }
    onData(cb: DataCb) {
        this.onDataCb = cb;
    }
    onMsg(msg: pb.Msg) {
        if (msg.command === pb.Msg.Command.NET_SOCKET_CONNECT_OK) {
            this.connectCb();
        } else if ( msg.command === pb.Msg.Command.NET_SOCKET_READ) {
            this.onDataCb(msg.netSocketData);
        }
    }
}

export function Socket(): NetSocket {
    const socket = new NetSocket();
    return socket;
}

export class NetServerConn {
    private onDataCb: DataCb;
    constructor(private readonly id: number) {
        this.id = id;
    }
    write(data: Uint8Array | string) {
        if (typeof(data) === "string") {
            data = enc.encode(data);   
        }
        pubInternal("net",{
            command: pb.Msg.Command.NET_SERVER_CLIENT_WRITE,
            netServerClientData: data,
            netServerClientId: this.id
        });
    }
    close() {
        pubInternal("net",{
            command: pb.Msg.Command.NET_SERVER_CLIENT_CLOSE,
            netServerClientId: this.id
        });
    }
    onData(cb: DataCb) {
        this.onDataCb = cb;
    }
    onMsg(msg: pb.Msg) {
        if ( msg.command === pb.Msg.Command.NET_SERVER_CLIENT_READ) {
            this.onDataCb(msg.netServerClientData);
        }
    }
  }

let nextServerId = 0;
export class NetServer {
    private readonly id: number;
    constructor(private connectCb: ConnCb) {
        this.id = nextServerId++;
        servers.set(this.id, this);
        this.connectCb = connectCb;
    }
    listen(port: number) {
        pubInternal("net", {
            command: pb.Msg.Command.NET_SERVER_LISTEN,
            netServerId: this.id,
            netServerPort: port
        });
    }
    close() {
        pubInternal("net", {
            command: pb.Msg.Command.NET_SERVER_CLOSE,
            netServerCloseServerId: this.id,
        });
    }
    private buildConn(msg: pb.Msg): NetServerConn {
        const conn = new NetServerConn(msg.netServerClientId);
        clients.set(msg.netServerClientId, conn);
        return conn;
    }
    onMsg(msg: pb.Msg) {
        const conn = this.buildConn(msg);
        this.connectCb(conn);
    }
}

export function createServer(cb: ConnCb): NetServer {
    const s = new NetServer(cb);
    return s;
}