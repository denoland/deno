import { signals } from "./signals.ts";

class StdoutReader {
  readonly #reader: ReadableStreamDefaultReader<string>;
  #text = "";

  constructor(stream: ReadableStream<Uint8Array>) {
    const textStream = stream.pipeThrough(new TextDecoderStream());
    this.#reader = textStream.getReader();
  }

  [Symbol.dispose]() {
    this.#reader.releaseLock();
  }

  async waitForText(waitingText: string) {
    if (this.#text.includes(waitingText)) {
      return;
    }

    while (true) {
      const { value, done } = await this.#reader.read();
      if (value) {
        this.#text += value;
        if (this.#text.includes(waitingText)) {
          break;
        }
      }
      if (done) {
        throw new Error("Did not find text: " + waitingText);
      }
    }
  }
}

const command = new Deno.Command(Deno.execPath(), {
  args: ["task", "listener"],
  stdout: "piped",
});

const child = command.spawn();
const reader = new StdoutReader(child.stdout!);
await reader.waitForText("Ready");

for (const signal of signals) {
  if (signal === "SIGTERM") {
    continue;
  }
  console.error("Sending", signal);
  child.kill(signal);
  await reader.waitForText("Received " + signal);
}

console.error("Sending SIGTERM");
child.kill("SIGTERM");
