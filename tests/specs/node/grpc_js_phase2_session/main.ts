import * as grpc from "npm:@grpc/grpc-js@1.14.3";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";

interface HelloRequest {
  name: string;
}

interface HelloReply {
  message: string;
}

interface ChunkReply {
  index: number;
  payload: string;
}

const LARGE_PAYLOAD = "x".repeat(96_000);

const encode = <T>(value: T): Buffer =>
  Buffer.from(JSON.stringify(value), "utf-8");

const decode = <T>(value: Buffer): T => JSON.parse(value.toString("utf-8"));

type GreeterHandlers = {
  SayHello: grpc.handleUnaryCall<HelloRequest, HelloReply>;
  StreamLarge: grpc.handleServerStreamingCall<HelloRequest, ChunkReply>;
};

const GreeterService: grpc.ServiceDefinition<GreeterHandlers> = {
  SayHello: {
    path: "/test.Greeter/SayHello",
    requestStream: false,
    responseStream: false,
    requestSerialize: encode<HelloRequest>,
    requestDeserialize: decode<HelloRequest>,
    responseSerialize: encode<HelloReply>,
    responseDeserialize: decode<HelloReply>,
  },
  StreamLarge: {
    path: "/test.Greeter/StreamLarge",
    requestStream: false,
    responseStream: true,
    requestSerialize: encode<HelloRequest>,
    requestDeserialize: decode<HelloRequest>,
    responseSerialize: encode<ChunkReply>,
    responseDeserialize: decode<ChunkReply>,
  },
};

type GreeterClient = grpc.Client & {
  SayHello(
    request: HelloRequest,
    callback: grpc.requestCallback<HelloReply>,
  ): grpc.ClientUnaryCall;
  StreamLarge(request: HelloRequest): grpc.ClientReadableStream<ChunkReply>;
};

const GreeterClientCtor = grpc.makeGenericClientConstructor(
  GreeterService,
  "Greeter",
) as unknown as {
  new (
    address: string,
    creds: grpc.ChannelCredentials,
    options?: grpc.ChannelOptions,
  ): GreeterClient;
};

async function startServer(): Promise<{ server: grpc.Server; port: number }> {
  const server = new grpc.Server();

  server.addService(GreeterService, {
    SayHello(
      call: grpc.ServerUnaryCall<HelloRequest, HelloReply>,
      callback: grpc.sendUnaryData<HelloReply>,
    ) {
      callback(null, { message: `Hello, ${call.request.name}` });
    },
    StreamLarge(call: grpc.ServerWritableStream<HelloRequest, ChunkReply>) {
      for (let index = 0; index < 3; index++) {
        call.write({ index, payload: LARGE_PAYLOAD });
      }
      call.end();
    },
  });

  const port = await new Promise<number>((resolve, reject) => {
    server.bindAsync(
      "127.0.0.1:0",
      grpc.ServerCredentials.createInsecure(),
      (error, boundPort) => {
        if (error) return reject(error);
        resolve(boundPort);
      },
    );
  });

  return { server, port };
}

function createClient(port: number): GreeterClient {
  return new GreeterClientCtor(
    `127.0.0.1:${port}`,
    grpc.credentials.createInsecure(),
    {
      "grpc-node.flow_control_window": 1_000_000,
      "grpc.keepalive_time_ms": 5,
      "grpc.keepalive_timeout_ms": 50,
      "grpc.keepalive_permit_without_calls": 1,
    },
  );
}

async function stopServer(server: grpc.Server): Promise<void> {
  await new Promise<void>((resolve) => server.tryShutdown(() => resolve()));
}

const { server, port } = await startServer();
const client = createClient(port);

try {
  await new Promise((resolve) => setTimeout(resolve, 20));

  await new Promise<void>((resolve, reject) => {
    client.SayHello({ name: "Phase2" }, (error, response) => {
      if (error) return reject(error);
      try {
        assert.equal(response?.message, "Hello, Phase2");
        resolve();
      } catch (assertionError) {
        reject(assertionError);
      }
    });
  });

  await new Promise<void>((resolve, reject) => {
    const call = client.StreamLarge({ name: "Phase2" });
    const chunks: ChunkReply[] = [];

    call.on("data", (chunk) => {
      chunks.push(chunk);
    });
    call.on("status", (status) => {
      try {
        assert.equal(status.code, grpc.status.OK);
        assert.deepStrictEqual(
          chunks.map((chunk) => chunk.index),
          [0, 1, 2],
        );
        for (const chunk of chunks) {
          assert.equal(chunk.payload.length, LARGE_PAYLOAD.length);
        }
        resolve();
      } catch (assertionError) {
        reject(assertionError);
      }
    });
    call.on("error", reject);
  });

  console.log("GRPC_PHASE2_OK");
} finally {
  client.close();
  await stopServer(server);
}
