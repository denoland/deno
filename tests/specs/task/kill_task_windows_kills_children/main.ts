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
if (isPidAlive(pid)) {
  throw new Error("Unexpected.");
}

function isPidAlive(pid: number) {
  const command = new Deno.Command("cmd", {
    args: ["/c", `wmic process where processid=${pid} get processid`],
  });

  try {
    const { stdout } = command.outputSync(); // Execute the command
    const output = new TextDecoder().decode(stdout);

    console.log("wmic output:", output.trim());
    return output.includes(pid.toString());
  } catch (error) {
    console.error("Error checking PID:", error);
    return false;
  }
}
