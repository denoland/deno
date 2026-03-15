import * as grpc from "npm:@grpc/grpc-js@1.14.3";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { readFileSync } from "node:fs";
import tls from "node:tls";

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

type GreeterHandlers = {
  SayHello: grpc.handleUnaryCall<HelloRequest, HelloReply>;
  SlowHello: grpc.handleUnaryCall<HelloRequest, HelloReply>;
  StreamLarge: grpc.handleServerStreamingCall<HelloRequest, ChunkReply>;
};

type GreeterClient = grpc.Client & {
  SayHello(
    request: HelloRequest,
    callback: grpc.requestCallback<HelloReply>,
  ): grpc.ClientUnaryCall;
  SayHello(
    request: HelloRequest,
    options: grpc.CallOptions,
    callback: grpc.requestCallback<HelloReply>,
  ): grpc.ClientUnaryCall;
  SlowHello(
    request: HelloRequest,
    callback: grpc.requestCallback<HelloReply>,
  ): grpc.ClientUnaryCall;
  SlowHello(
    request: HelloRequest,
    options: grpc.CallOptions,
    callback: grpc.requestCallback<HelloReply>,
  ): grpc.ClientUnaryCall;
  StreamLarge(request: HelloRequest): grpc.ClientReadableStream<ChunkReply>;
};

const LARGE_PAYLOAD = "x".repeat(96_000);

const encode = <T>(value: T): Buffer =>
  Buffer.from(JSON.stringify(value), "utf-8");
const decode = <T>(value: Buffer): T => JSON.parse(value.toString("utf-8"));

const ROOT_CA = readFileSync(
  new URL("../../../testdata/tls/RootCA.crt", import.meta.url),
);
const SERVER_CERT = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const SERVER_KEY = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

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
  SlowHello: {
    path: "/test.Greeter/SlowHello",
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

function withTimeout<T>(
  promise: Promise<T>,
  label: string,
  ms = 7_000,
): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`${label} timed out`)), ms)
    ),
  ]);
}

const GreeterClientCtor = grpc.makeGenericClientConstructor(
  GreeterService,
  "GreeterTls",
) as unknown as {
  new (
    address: string,
    creds: grpc.ChannelCredentials,
    options?: grpc.ChannelOptions,
  ): GreeterClient;
};

function createClient(
  host: string,
  port: number,
  creds = grpc.credentials.createSsl(ROOT_CA),
  targetNameOverride = "localhost",
): GreeterClient {
  return new GreeterClientCtor(`${host}:${port}`, creds, {
    "grpc-node.flow_control_window": 1_000_000,
    "grpc.keepalive_time_ms": 5,
    "grpc.keepalive_timeout_ms": 100,
    "grpc.keepalive_permit_without_calls": 1,
    ...(targetNameOverride
      ? { "grpc.ssl_target_name_override": targetNameOverride }
      : {}),
  });
}

async function startServer(): Promise<{ server: grpc.Server; port: number }> {
  const server = new grpc.Server();

  server.addService(GreeterService, {
    SayHello(
      call: grpc.ServerUnaryCall<HelloRequest, HelloReply>,
      callback: grpc.sendUnaryData<HelloReply>,
    ) {
      callback(null, { message: `Hello, ${call.request.name}` });
    },
    SlowHello(
      call: grpc.ServerUnaryCall<HelloRequest, HelloReply>,
      callback: grpc.sendUnaryData<HelloReply>,
    ) {
      setTimeout(() => {
        callback(null, { message: `Slow hello, ${call.request.name}` });
      }, 150);
    },
    StreamLarge(
      call: grpc.ServerWritableStream<HelloRequest, ChunkReply>,
    ) {
      for (let index = 0; index < 3; index++) {
        call.write({ index, payload: LARGE_PAYLOAD });
      }
      call.end();
    },
  });

  const port = await withTimeout(
    new Promise<number>((resolve, reject) => {
      server.bindAsync(
        "localhost:0",
        grpc.ServerCredentials.createSsl(null, [{
          private_key: SERVER_KEY,
          cert_chain: SERVER_CERT,
        }], false),
        (error, boundPort) => {
          if (error) return reject(error);
          resolve(boundPort);
        },
      );
    }),
    "secure bindAsync",
  );

  return { server, port };
}

async function stopServer(server: grpc.Server): Promise<void> {
  await new Promise<void>((resolve) => server.tryShutdown(() => resolve()));
}

function unary(
  client: GreeterClient,
  request: HelloRequest,
  options?: grpc.CallOptions,
): Promise<HelloReply> {
  return new Promise<HelloReply>((resolve, reject) => {
    const cb: grpc.requestCallback<HelloReply> = (error, response) => {
      if (error) return reject(error);
      if (!response) return reject(new Error("missing unary response"));
      resolve(response);
    };
    if (options) {
      client.SayHello(request, options, cb);
    } else {
      client.SayHello(request, cb);
    }
  });
}

function unaryUnknownMethod(port: number): Promise<grpc.ServiceError> {
  const unknownService: grpc.ServiceDefinition = {
    NoSuchMethod: {
      path: "/test.Greeter/NoSuchMethod",
      requestStream: false,
      responseStream: false,
      requestSerialize: encode,
      requestDeserialize: decode,
      responseSerialize: encode,
      responseDeserialize: decode,
    },
  };
  const UnknownClientCtor = grpc.makeGenericClientConstructor(
    unknownService,
    "UnknownTls",
  ) as unknown as {
    new (
      address: string,
      creds: grpc.ChannelCredentials,
    ): grpc.Client & {
      NoSuchMethod(
        request: unknown,
        callback: grpc.requestCallback<unknown>,
      ): grpc.ClientUnaryCall;
    };
  };
  const client = new UnknownClientCtor(
    `localhost:${port}`,
    grpc.credentials.createSsl(ROOT_CA),
  );
  return new Promise<grpc.ServiceError>((resolve) => {
    client.NoSuchMethod({}, (error) => {
      client.close();
      resolve(error!);
    });
  });
}

const { server, port } = await startServer();
const client = createClient("localhost", port);

try {
  await withTimeout(
    new Promise<void>((resolve, reject) => {
      const probe = tls.connect({
        host: "localhost",
        port,
        ca: ROOT_CA,
        servername: "localhost",
        ALPNProtocols: ["h2"],
        rejectUnauthorized: true,
      }, () => {
        probe.destroy();
        resolve();
      });
      probe.on("error", reject);
    }),
    "TLS probe",
  );

  const unaryReply = await withTimeout(
    unary(client, { name: "TLS" }),
    "TLS unary",
  );
  assert.equal(unaryReply.message, "Hello, TLS");
  console.log("TLS_UNARY_OK");

  await withTimeout(
    new Promise<void>((resolve, reject) => {
      const stream = client.StreamLarge({ name: "TLS" });
      const chunks: ChunkReply[] = [];

      stream.on("data", (chunk) => {
        chunks.push(chunk);
      });
      stream.on("status", (status) => {
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
        } catch (error) {
          reject(error);
        }
      });
      stream.on("error", reject);
    }),
    "TLS stream",
  );
  console.log("TLS_STREAM_OK");

  const unknownError = await withTimeout(
    unaryUnknownMethod(port),
    "TLS unknown method",
  );
  assert.equal(unknownError.code, grpc.status.UNIMPLEMENTED);
  console.log("TLS_UNKNOWN_OK");

  await withTimeout(
    new Promise<void>((resolve, reject) => {
      const call = client.SlowHello({ name: "cancel-me" }, (error) => {
        try {
          assert.ok(error);
          assert.equal(error!.code, grpc.status.CANCELLED);
          resolve();
        } catch (assertionError) {
          reject(assertionError);
        }
      });
      setTimeout(() => call.cancel(), 10);
    }),
    "TLS cancel",
  );
  console.log("TLS_CANCEL_OK");

  await withTimeout(
    new Promise<void>((resolve, reject) => {
      const deadline = Date.now() + 40;
      client.SlowHello(
        { name: "deadline" },
        { deadline },
        (error, _response) => {
          try {
            assert.ok(error);
            assert.equal(error!.code, grpc.status.DEADLINE_EXCEEDED);
            resolve();
          } catch (assertionError) {
            reject(assertionError);
          }
        },
      );
    }),
    "TLS deadline",
  );
  console.log("TLS_DEADLINE_OK");

  const wrongHostClient = createClient(
    "localhost",
    port,
    undefined,
    "definitely-not-localhost",
  );
  await withTimeout(
    new Promise<void>((resolve, reject) => {
      wrongHostClient.SayHello(
        { name: "wrong-hostname" },
        { deadline: Date.now() + 1_500 },
        (error, _response) => {
          try {
            assert.ok(error);
            assert.equal(error!.code, grpc.status.UNAVAILABLE);
            resolve();
          } catch (assertionError) {
            reject(assertionError);
          }
        },
      );
    }),
    "TLS bad hostname",
  );
  wrongHostClient.close();
  console.log("TLS_BAD_HOSTNAME_OK");

  console.log("TLS_DONE");
} finally {
  client.close();
  await stopServer(server);
}
