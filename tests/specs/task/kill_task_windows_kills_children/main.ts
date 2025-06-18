import process from "node:process";

class StdoutReader {
  readonly #reader: ReadableStreamDefaultReader<string>;
  text = "";

  constructor(stream: ReadableStream<Uint8Array>) {
    const textStream = stream.pipeThrough(new TextDecoderStream());
    this.#reader = textStream.getReader();
  }

  [Symbol.dispose]() {
    this.#reader.releaseLock();
  }

  async waitForText(waitingText: string) {
    if (this.text.includes(waitingText)) {
      return;
    }

    while (true) {
      const { value, done } = await this.#reader.read();
      if (value) {
        this.text += value;
        if (this.text.includes(waitingText)) {
          break;
        }
      }
      if (done) {
        throw new Error("Did not find text: " + waitingText);
      }
    }
  }
}

const command = new Deno.Command("deno", {
  args: ["task", "start"],
  stdout: "piped",
});

const child = command.spawn();

const reader = new StdoutReader(child.stdout!);
console.log("Waiting...");
await reader.waitForText("Ready");
console.log("Received.");
const pid = parseInt(reader.text.split("\n")[0], 10);
console.log("PID", pid);
// ensure this function works
if (!isPidAlive(child.pid)) {
  throw new Error("Unexpected.");
}
if (!isPidAlive(pid)) {
  throw new Error("Unexpected.");
}
child.kill();
// now the grandchild shouldn't be alive
if (isPidAlive(child.pid)) {
  throw new Error("Unexpected.");
}

let stillAlive = true;
for (let i = 0; i < 20; i++) {
  if (!isPidAlive(pid)) {
    stillAlive = false;
    break;
  }
  await new Promise((resolve) => setTimeout(resolve, 50));
}

if (stillAlive) {
  throw new Error("Unexpected.");
}

function isPidAlive(pid: number) {
  try {
    return process.kill(pid, 0);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ESRCH") {
      return false;
    }
    console.log("Error checking PID:", error);
    throw error;
  }
}
