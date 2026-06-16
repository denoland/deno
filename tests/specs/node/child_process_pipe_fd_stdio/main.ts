import { spawn } from "node:child_process";

type ReadableStreamLike = {
  setEncoding(encoding: BufferEncoding): void;
  on(event: "data", listener: (chunk: string) => void): void;
  on(event: "end", listener: () => void): void;
  on(event: "error", listener: (error: Error) => void): void;
};

function waitClose(child: ReturnType<typeof spawn>): Promise<number | null> {
  return new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", (code) => resolve(code));
  });
}

function collect(stream: ReadableStreamLike | null): Promise<string> {
  if (stream === null) {
    throw new Error("expected stream");
  }

  let output = "";
  stream.setEncoding("utf8");
  stream.on("data", (chunk) => {
    output += chunk;
  });

  return new Promise((resolve, reject) => {
    stream.on("error", reject);
    stream.on("end", () => resolve(output));
  });
}

async function runPipeline(kind: "stream" | "fd") {
  const producer = spawn("seq", ["1", "100000"], {
    stdio: ["ignore", "pipe", "inherit"],
  });

  let stdin: NonNullable<typeof producer.stdout> | number = producer.stdout!;
  if (kind === "fd") {
    const fd = (producer.stdout as unknown as { _handle?: { fd?: unknown } })
      ._handle
      ?.fd;
    if (typeof fd !== "number") {
      throw new Error("expected producer stdout fd");
    }
    stdin = fd;
  }

  const consumer = spawn("wc", ["-l"], {
    stdio: [stdin, "pipe", "pipe"],
  });

  const [stdout, stderr, consumerCode, producerCode] = await Promise.all([
    collect(consumer.stdout),
    collect(consumer.stderr),
    waitClose(consumer),
    waitClose(producer),
  ]);

  console.log(`${kind} stdout: ${stdout.trim()}`);
  console.log(`${kind} stderr: ${stderr.trim()}`);
  console.log(`${kind} consumer exit: ${consumerCode}`);
  console.log(`${kind} producer exit: ${producerCode}`);
}

await runPipeline("stream");
await runPipeline("fd");
