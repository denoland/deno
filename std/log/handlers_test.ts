// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import {
  assert,
  assertEquals,
  assertThrowsAsync,
  assertNotEquals,
} from "../testing/asserts.ts";
import {
  LogLevels,
  LogLevelNames,
  getLevelName,
  getLevelByName,
  LevelName,
} from "./levels.ts";
import { BaseHandler, FileHandler, RotatingFileHandler } from "./handlers.ts";
import { LogRecord } from "./logger.ts";
import { existsSync } from "../fs/exists.ts";

const LOG_FILE = "./test_log.file";

class TestHandler extends BaseHandler {
  public messages: string[] = [];

  public log(str: string): void {
    this.messages.push(str);
  }
}

test("simpleHandler", function (): void {
  const cases = new Map<number, string[]>([
    [
      LogLevels.DEBUG,
      [
        "DEBUG debug-test",
        "INFO info-test",
        "WARNING warning-test",
        "ERROR error-test",
        "CRITICAL critical-test",
      ],
    ],
    [
      LogLevels.INFO,
      [
        "INFO info-test",
        "WARNING warning-test",
        "ERROR error-test",
        "CRITICAL critical-test",
      ],
    ],
    [
      LogLevels.WARNING,
      ["WARNING warning-test", "ERROR error-test", "CRITICAL critical-test"],
    ],
    [LogLevels.ERROR, ["ERROR error-test", "CRITICAL critical-test"]],
    [LogLevels.CRITICAL, ["CRITICAL critical-test"]],
  ]);

  for (const [testCase, messages] of cases.entries()) {
    const testLevel = getLevelName(testCase);
    const handler = new TestHandler(testLevel);

    for (const levelName of LogLevelNames) {
      const level = getLevelByName(levelName as LevelName);
      handler.handle(
        new LogRecord(`${levelName.toLowerCase()}-test`, [], level)
      );
    }

    assertEquals(handler.level, testCase);
    assertEquals(handler.levelName, testLevel);
    assertEquals(handler.messages, messages);
  }
});

test("testFormatterAsString", function (): void {
  const handler = new TestHandler("DEBUG", {
    formatter: "test {levelName} {msg}",
  });

  handler.handle(new LogRecord("Hello, world!", [], LogLevels.DEBUG));

  assertEquals(handler.messages, ["test DEBUG Hello, world!"]);
});

test("testFormatterAsFunction", function (): void {
  const handler = new TestHandler("DEBUG", {
    formatter: (logRecord): string =>
      `fn formatter ${logRecord.levelName} ${logRecord.msg}`,
  });

  handler.handle(new LogRecord("Hello, world!", [], LogLevels.ERROR));

  assertEquals(handler.messages, ["fn formatter ERROR Hello, world!"]);
});

test({
  name: "FileHandler with mode 'w' will wipe clean existing log file",
  async fn() {
    const fileHandler = new FileHandler("WARNING", {
      filename: LOG_FILE,
      mode: "w",
    });

    await fileHandler.setup();
    fileHandler.handle(new LogRecord("Hello World", [], LogLevels.WARNING));
    await fileHandler.destroy();
    const firstFileSize = (await Deno.stat(LOG_FILE)).size;

    await fileHandler.setup();
    fileHandler.handle(new LogRecord("Hello World", [], LogLevels.WARNING));
    await fileHandler.destroy();
    const secondFileSize = (await Deno.stat(LOG_FILE)).size;

    assertEquals(secondFileSize, firstFileSize);
    Deno.removeSync(LOG_FILE);
  },
});

test({
  name: "FileHandler with mode 'x' will throw if log file already exists",
  async fn() {
    const fileHandler = new FileHandler("WARNING", {
      filename: LOG_FILE,
      mode: "x",
    });
    Deno.writeFileSync(LOG_FILE, new TextEncoder().encode("hello world"));

    await assertThrowsAsync(async () => {
      await fileHandler.setup();
    }, Deno.errors.AlreadyExists);

    await fileHandler.destroy();

    Deno.removeSync(LOG_FILE);
  },
});

test({
  name:
    "RotatingFileHandler with mode 'w' will wipe clean existing log file and remove others",
  async fn() {
    Deno.writeFileSync(LOG_FILE, new TextEncoder().encode("hello world"));
    Deno.writeFileSync(
      LOG_FILE + ".1",
      new TextEncoder().encode("hello world")
    );
    Deno.writeFileSync(
      LOG_FILE + ".2",
      new TextEncoder().encode("hello world")
    );
    Deno.writeFileSync(
      LOG_FILE + ".3",
      new TextEncoder().encode("hello world")
    );

    const fileHandler = new RotatingFileHandler("WARNING", {
      filename: LOG_FILE,
      maxBytes: 50,
      maxBackupCount: 3,
      mode: "w",
    });
    await fileHandler.setup();
    await fileHandler.destroy();

    assertEquals((await Deno.stat(LOG_FILE)).size, 0);
    assert(!existsSync(LOG_FILE + ".1"));
    assert(!existsSync(LOG_FILE + ".2"));
    assert(!existsSync(LOG_FILE + ".3"));

    Deno.removeSync(LOG_FILE);
  },
});

test({
  name:
    "RotatingFileHandler with mode 'x' will throw if any log file already exists",
  async fn() {
    Deno.writeFileSync(
      LOG_FILE + ".3",
      new TextEncoder().encode("hello world")
    );
    const fileHandler = new RotatingFileHandler("WARNING", {
      filename: LOG_FILE,
      maxBytes: 50,
      maxBackupCount: 3,
      mode: "x",
    });
    await assertThrowsAsync(
      async () => {
        await fileHandler.setup();
      },
      Deno.errors.AlreadyExists,
      "Backup log file " + LOG_FILE + ".3 already exists"
    );

    fileHandler.destroy();
    Deno.removeSync(LOG_FILE + ".3");
    Deno.removeSync(LOG_FILE);
  },
});

test({
  name: "RotatingFileHandler with first rollover, monitor step by step",
  async fn() {
    const fileHandler = new RotatingFileHandler("WARNING", {
      filename: LOG_FILE,
      maxBytes: 25,
      maxBackupCount: 3,
      mode: "w",
    });
    await fileHandler.setup();

    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR)); // 'ERROR AAA\n' = 10 bytes
    fileHandler.flush();
    assertEquals((await Deno.stat(LOG_FILE)).size, 10);
    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR));
    fileHandler.flush();
    assertEquals((await Deno.stat(LOG_FILE)).size, 20);
    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR));
    fileHandler.flush();
    // Rollover occurred. Log file now has 1 record, rollover file has the original 2
    assertEquals((await Deno.stat(LOG_FILE)).size, 10);
    assertEquals((await Deno.stat(LOG_FILE + ".1")).size, 20);
    await fileHandler.destroy();

    Deno.removeSync(LOG_FILE);
    Deno.removeSync(LOG_FILE + ".1");
  },
});

test({
  name: "RotatingFileHandler with first rollover, check all at once",
  async fn() {
    const fileHandler = new RotatingFileHandler("WARNING", {
      filename: LOG_FILE,
      maxBytes: 25,
      maxBackupCount: 3,
      mode: "w",
    });
    await fileHandler.setup();

    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR)); // 'ERROR AAA\n' = 10 bytes
    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR));
    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR));

    await fileHandler.destroy();

    assertEquals((await Deno.stat(LOG_FILE)).size, 10);
    assertEquals((await Deno.stat(LOG_FILE + ".1")).size, 20);

    Deno.removeSync(LOG_FILE);
    Deno.removeSync(LOG_FILE + ".1");
  },
});

test({
  name: "RotatingFileHandler with all backups rollover",
  async fn() {
    Deno.writeFileSync(LOG_FILE, new TextEncoder().encode("original log file"));
    Deno.writeFileSync(
      LOG_FILE + ".1",
      new TextEncoder().encode("original log.1 file")
    );
    Deno.writeFileSync(
      LOG_FILE + ".2",
      new TextEncoder().encode("original log.2 file")
    );
    Deno.writeFileSync(
      LOG_FILE + ".3",
      new TextEncoder().encode("original log.3 file")
    );

    const fileHandler = new RotatingFileHandler("WARNING", {
      filename: LOG_FILE,
      maxBytes: 2,
      maxBackupCount: 3,
      mode: "a",
    });
    await fileHandler.setup();
    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR)); // 'ERROR AAA\n' = 10 bytes
    await fileHandler.destroy();

    const decoder = new TextDecoder();
    assertEquals(decoder.decode(Deno.readFileSync(LOG_FILE)), "ERROR AAA\n");
    assertEquals(
      decoder.decode(Deno.readFileSync(LOG_FILE + ".1")),
      "original log file"
    );
    assertEquals(
      decoder.decode(Deno.readFileSync(LOG_FILE + ".2")),
      "original log.1 file"
    );
    assertEquals(
      decoder.decode(Deno.readFileSync(LOG_FILE + ".3")),
      "original log.2 file"
    );
    assert(!existsSync(LOG_FILE + ".4"));

    Deno.removeSync(LOG_FILE);
    Deno.removeSync(LOG_FILE + ".1");
    Deno.removeSync(LOG_FILE + ".2");
    Deno.removeSync(LOG_FILE + ".3");
  },
});

test({
  name: "RotatingFileHandler maxBytes cannot be less than 1",
  async fn() {
    await assertThrowsAsync(
      async () => {
        const fileHandler = new RotatingFileHandler("WARNING", {
          filename: LOG_FILE,
          maxBytes: 0,
          maxBackupCount: 3,
          mode: "w",
        });
        await fileHandler.setup();
      },
      Error,
      "maxBytes cannot be less than 1"
    );
  },
});

test({
  name: "RotatingFileHandler maxBackupCount cannot be less than 1",
  async fn() {
    await assertThrowsAsync(
      async () => {
        const fileHandler = new RotatingFileHandler("WARNING", {
          filename: LOG_FILE,
          maxBytes: 50,
          maxBackupCount: 0,
          mode: "w",
        });
        await fileHandler.setup();
      },
      Error,
      "maxBackupCount cannot be less than 1"
    );
  },
});

test({
  name: "Window unload flushes buffer",
  async fn() {
    const fileHandler = new FileHandler("WARNING", {
      filename: LOG_FILE,
      mode: "w",
    });
    await fileHandler.setup();
    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR)); // 'ERROR AAA\n' = 10 bytes

    assertEquals((await Deno.stat(LOG_FILE)).size, 0);
    dispatchEvent(new Event("unload"));
    assertEquals((await Deno.stat(LOG_FILE)).size, 10);

    Deno.removeSync(LOG_FILE);
  },
});

test({
  name: "RotatingFileHandler: rotate on byte length, not msg length",
  async fn() {
    const fileHandler = new RotatingFileHandler("WARNING", {
      filename: LOG_FILE,
      maxBytes: 7,
      maxBackupCount: 1,
      mode: "w",
    });
    await fileHandler.setup();

    const msg = "。";
    const msgLength = msg.length;
    const msgByteLength = new TextEncoder().encode(msg).byteLength;
    assertNotEquals(msgLength, msgByteLength);
    assertEquals(msgLength, 1);
    assertEquals(msgByteLength, 3);

    fileHandler.log(msg); // logs 4 bytes (including '\n')
    fileHandler.log(msg); // max bytes is 7, but this would be 8.  Rollover.

    await fileHandler.destroy();

    const fileSize1 = (await Deno.stat(LOG_FILE)).size;
    const fileSize2 = (await Deno.stat(LOG_FILE + ".1")).size;

    assertEquals(fileSize1, msgByteLength + 1);
    assertEquals(fileSize2, msgByteLength + 1);

    Deno.removeSync(LOG_FILE);
    Deno.removeSync(LOG_FILE + ".1");
  },
});

test({
  name: "FileHandler: Critical logs trigger immediate flush",
  async fn() {
    const fileHandler = new FileHandler("WARNING", {
      filename: LOG_FILE,
      mode: "w",
    });
    await fileHandler.setup();

    fileHandler.handle(new LogRecord("AAA", [], LogLevels.ERROR));

    // ERROR won't trigger immediate flush
    const fileSize = (await Deno.stat(LOG_FILE)).size;
    assertEquals(fileSize, 0);

    fileHandler.handle(new LogRecord("AAA", [], LogLevels.CRITICAL));

    // CRITICAL will trigger immediate flush
    const fileSize2 = (await Deno.stat(LOG_FILE)).size;
    // ERROR record is 10 bytes, CRITICAL is 13 bytes
    assertEquals(fileSize2, 23);

    await fileHandler.destroy();
    Deno.removeSync(LOG_FILE);
  },
});
