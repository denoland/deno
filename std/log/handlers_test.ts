// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertThrows,
} from "../testing/asserts.ts";

import { FileHandler, Handler, RotatingFileHandler } from "./handlers.ts";
import { LogRecord } from "./logger.ts";
import { existsSync } from "../fs/exists.ts";
import { LogLevel, logLevels } from "./levels.ts";

const LOG_FILE = "./test_log.file";

class TestHandler extends Handler {
  public messages: string[] = [];

  handlerFunctions = {
    [logLevels.trace.code]: (message: string) => this.messages.push(message),
    [logLevels.debug.code]: (message: string) => this.messages.push(message),
    [logLevels.info.code]: (message: string) => this.messages.push(message),
    [logLevels.warn.code]: (message: string) => this.messages.push(message),
    [logLevels.error.code]: (message: string) => this.messages.push(message),
  };
}

Deno.test("simpleHandler", function (): void {
  const cases = new Map<LogLevel, string[]>([
    [
      logLevels.trace,
      [
        "Trace trace-test",
        "Debug debug-test",
        "Info info-test",
        "Warn warn-test",
        "Error error-test",
      ],
    ],
    [
      logLevels.debug,
      [
        "Debug debug-test",
        "Info info-test",
        "Warn warn-test",
        "Error error-test",
      ],
    ],
    [
      logLevels.info,
      [
        "Info info-test",
        "Warn warn-test",
        "Error error-test",
      ],
    ],
    [
      logLevels.warn,
      ["Warn warn-test", "Error error-test"],
    ],
    [logLevels.error, ["Error error-test"]],
  ]);

  for (const [logLevel, messages] of cases.entries()) {
    const handler = new TestHandler(logLevel);

    for (const logLevel of Object.values(logLevels)) {
      handler.handle(
        new LogRecord({
          loggerName: "default",
          message: `${logLevel.name.toLowerCase()}-test`,
          args: [],
          logLevel,
        }),
      );
    }

    assertEquals(handler.logLevel, logLevel);
    assertEquals(handler.messages, messages);
  }
});

Deno.test("formatter asString", function (): void {
  const handler = new TestHandler(logLevels.debug, {
    formatter: ({ logLevel, message }) => `test ${logLevel.name} ${message}`,
  });

  handler.handle(
    new LogRecord({
      loggerName: "default",
      message: "Hello, world!",
      args: [],
      logLevel: logLevels.debug,
    }),
  );

  assertEquals(handler.messages, ["test Debug Hello, world!"]);
});

Deno.test("formatter WithEmptymessage", function () {
  const handler = new TestHandler(logLevels.debug, {
    formatter: ({ logLevel, message }) => `test ${logLevel.name} ${message}`,
  });

  handler.handle(
    new LogRecord({
      loggerName: "default",
      message: "",
      args: [],
      logLevel: logLevels.debug,
    }),
  );

  assertEquals(handler.messages, ["test Debug "]);
});

Deno.test("formatter AsFunction", function (): void {
  const handler = new TestHandler(logLevels.debug, {
    formatter: (logRecord): string =>
      `fn formatter ${logRecord.logLevel.name} ${logRecord.message}`,
  });

  handler.handle(
    new LogRecord({
      loggerName: "default",
      message: "Hello, world!",
      args: [],
      logLevel: logLevels.error,
    }),
  );

  assertEquals(handler.messages, ["fn formatter Error Hello, world!"]);
});

Deno.test({
  name: "FileHandler with mode 'w' will wipe clean existing log file",
  async fn() {
    const fileHandler = new FileHandler(logLevels.warn, {
      filename: LOG_FILE,
      mode: "w",
    });

    fileHandler.open();
    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "Hello World",
        args: [],
        logLevel: logLevels.warn,
      }),
    );
    fileHandler.close();
    const firstFileSize = (await Deno.stat(LOG_FILE)).size;

    fileHandler.open();
    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "Hello World",
        args: [],
        logLevel: logLevels.warn,
      }),
    );
    fileHandler.close();
    const secondFileSize = (await Deno.stat(LOG_FILE)).size;

    assertEquals(secondFileSize, firstFileSize);
    Deno.removeSync(LOG_FILE);
  },
});

Deno.test({
  name: "FileHandler with mode 'x' will throw if log file already exists",
  fn() {
    const fileHandler = new FileHandler(logLevels.warn, {
      filename: LOG_FILE,
      mode: "x",
    });
    Deno.writeFileSync(LOG_FILE, new TextEncoder().encode("hello world"));

    assertThrows(() => {
      fileHandler.open();
    }, Deno.errors.AlreadyExists);

    fileHandler.close();

    Deno.removeSync(LOG_FILE);
  },
});

Deno.test({
  name:
    "RotatingFileHandler with mode 'w' will wipe clean existing log file and remove others",
  async fn() {
    Deno.writeFileSync(LOG_FILE, new TextEncoder().encode("hello world"));
    Deno.writeFileSync(
      LOG_FILE + ".1",
      new TextEncoder().encode("hello world"),
    );
    Deno.writeFileSync(
      LOG_FILE + ".2",
      new TextEncoder().encode("hello world"),
    );
    Deno.writeFileSync(
      LOG_FILE + ".3",
      new TextEncoder().encode("hello world"),
    );

    const fileHandler = new RotatingFileHandler(logLevels.warn, {
      filename: LOG_FILE,
      maxBytes: 50,
      maxBackupCount: 3,
      mode: "w",
    });
    fileHandler.open();
    fileHandler.close();

    assertEquals((await Deno.stat(LOG_FILE)).size, 0);
    assert(!existsSync(LOG_FILE + ".1"));
    assert(!existsSync(LOG_FILE + ".2"));
    assert(!existsSync(LOG_FILE + ".3"));

    Deno.removeSync(LOG_FILE);
  },
});

Deno.test({
  name:
    "RotatingFileHandler with mode 'x' will throw if any log file already exists",
  fn() {
    Deno.writeFileSync(
      LOG_FILE + ".3",
      new TextEncoder().encode("hello world"),
    );
    const fileHandler = new RotatingFileHandler(logLevels.warn, {
      filename: LOG_FILE,
      maxBytes: 50,
      maxBackupCount: 3,
      mode: "x",
    });
    assertThrows(
      () => {
        fileHandler.open();
      },
      Deno.errors.AlreadyExists,
      "Backup log file " + LOG_FILE + ".3 already exists",
    );

    fileHandler.close();
    Deno.removeSync(LOG_FILE + ".3");
    Deno.removeSync(LOG_FILE);
  },
});

Deno.test({
  name: "RotatingFileHandler with first rollover, monitor step by step",
  async fn() {
    const fileHandler = new RotatingFileHandler(logLevels.warn, {
      filename: LOG_FILE,
      maxBytes: 25,
      maxBackupCount: 3,
      mode: "w",
    });
    fileHandler.open();

    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "AAA",
        args: [],
        logLevel: logLevels.error,
      }),
    ); // 'Error AAA\n' = 10 bytes
    fileHandler.flush();
    assertEquals((await Deno.stat(LOG_FILE)).size, 10);
    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "AAA",
        args: [],
        logLevel: logLevels.error,
      }),
    );
    fileHandler.flush();
    assertEquals((await Deno.stat(LOG_FILE)).size, 20);
    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "AAA",
        args: [],
        logLevel: logLevels.error,
      }),
    );
    fileHandler.flush();
    // Rollover occurred. Log file now has 1 record, rollover file has the original 2
    assertEquals((await Deno.stat(LOG_FILE)).size, 10);
    assertEquals((await Deno.stat(LOG_FILE + ".1")).size, 20);
    fileHandler.close();

    Deno.removeSync(LOG_FILE);
    Deno.removeSync(LOG_FILE + ".1");
  },
});

Deno.test({
  name: "RotatingFileHandler with first rollover, check all at once",
  async fn() {
    const fileHandler = new RotatingFileHandler(logLevels.warn, {
      filename: LOG_FILE,
      maxBytes: 25,
      maxBackupCount: 3,
      mode: "w",
    });
    fileHandler.open();

    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "AAA",
        args: [],
        logLevel: logLevels.error,
      }),
    ); // 'Error AAA\n' = 10 bytes
    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "AAA",
        args: [],
        logLevel: logLevels.error,
      }),
    );
    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "AAA",
        args: [],
        logLevel: logLevels.error,
      }),
    );

    fileHandler.close();

    assertEquals((await Deno.stat(LOG_FILE)).size, 10);
    assertEquals((await Deno.stat(LOG_FILE + ".1")).size, 20);

    Deno.removeSync(LOG_FILE);
    Deno.removeSync(LOG_FILE + ".1");
  },
});

Deno.test({
  name: "RotatingFileHandler with all backups rollover",
  fn() {
    Deno.writeFileSync(LOG_FILE, new TextEncoder().encode("original log file"));
    Deno.writeFileSync(
      LOG_FILE + ".1",
      new TextEncoder().encode("original log.1 file"),
    );
    Deno.writeFileSync(
      LOG_FILE + ".2",
      new TextEncoder().encode("original log.2 file"),
    );
    Deno.writeFileSync(
      LOG_FILE + ".3",
      new TextEncoder().encode("original log.3 file"),
    );

    const fileHandler = new RotatingFileHandler(logLevels.warn, {
      filename: LOG_FILE,
      maxBytes: 2,
      maxBackupCount: 3,
      mode: "a",
    });
    fileHandler.open();
    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "AAA",
        args: [],
        logLevel: logLevels.error,
      }),
    ); // 'Error AAA\n' = 10 bytes
    fileHandler.close();

    const decoder = new TextDecoder();
    assertEquals(decoder.decode(Deno.readFileSync(LOG_FILE)), "Error AAA\n");
    assertEquals(
      decoder.decode(Deno.readFileSync(LOG_FILE + ".1")),
      "original log file",
    );
    assertEquals(
      decoder.decode(Deno.readFileSync(LOG_FILE + ".2")),
      "original log.1 file",
    );
    assertEquals(
      decoder.decode(Deno.readFileSync(LOG_FILE + ".3")),
      "original log.2 file",
    );
    assert(!existsSync(LOG_FILE + ".4"));

    Deno.removeSync(LOG_FILE);
    Deno.removeSync(LOG_FILE + ".1");
    Deno.removeSync(LOG_FILE + ".2");
    Deno.removeSync(LOG_FILE + ".3");
  },
});

Deno.test({
  name: "RotatingFileHandler maxBytes cannot be less than 1",
  fn() {
    assertThrows(
      () => {
        const fileHandler = new RotatingFileHandler(logLevels.warn, {
          filename: LOG_FILE,
          maxBytes: 0,
          maxBackupCount: 3,
          mode: "w",
        });
        fileHandler.open();
      },
      Error,
      "maxBytes cannot be less than 1",
    );
  },
});

Deno.test({
  name: "RotatingFileHandler maxBackupCount cannot be less than 1",
  fn() {
    assertThrows(
      () => {
        const fileHandler = new RotatingFileHandler(logLevels.warn, {
          filename: LOG_FILE,
          maxBytes: 50,
          maxBackupCount: 0,
          mode: "w",
        });
        fileHandler.open();
      },
      Error,
      "maxBackupCount cannot be less than 1",
    );
  },
});

Deno.test({
  name: "Window unload flushes buffer",
  async fn() {
    const fileHandler = new FileHandler(logLevels.warn, {
      filename: LOG_FILE,
      mode: "w",
    });
    fileHandler.open();
    fileHandler.handle(
      new LogRecord({
        loggerName: "default",
        message: "AAA",
        args: [],
        logLevel: logLevels.error,
      }),
    ); // 'Error AAA\n' = 10 bytes

    assertEquals((await Deno.stat(LOG_FILE)).size, 0);
    dispatchEvent(new Event("unload"));
    assertEquals((await Deno.stat(LOG_FILE)).size, 10);

    Deno.removeSync(LOG_FILE);
  },
});

Deno.test({
  name: "RotatingFileHandler: rotate on byte length, not message length",
  async fn() {
    const fileHandler = new RotatingFileHandler(logLevels.warn, {
      filename: LOG_FILE,
      maxBytes: 7,
      maxBackupCount: 1,
      mode: "w",
    });
    fileHandler.open();

    const message = "。";
    const messageLength = message.length;
    const messageByteLength = new TextEncoder().encode(message).byteLength;
    assertNotEquals(messageLength, messageByteLength);
    assertEquals(messageLength, 1);
    assertEquals(messageByteLength, 3);

    fileHandler.write(message); // logs 4 bytes (including '\n')
    fileHandler.write(message); // max bytes is 7, but this would be 8.  Rollover.

    fileHandler.close();

    const fileSize1 = (await Deno.stat(LOG_FILE)).size;
    const fileSize2 = (await Deno.stat(LOG_FILE + ".1")).size;

    assertEquals(fileSize1, messageByteLength + 1);
    assertEquals(fileSize2, messageByteLength + 1);

    Deno.removeSync(LOG_FILE);
    Deno.removeSync(LOG_FILE + ".1");
  },
});
