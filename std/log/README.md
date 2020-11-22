# Log

## Usage

```ts
import {
  ConsoleHandler,
  Logger,
  logLevels,
} from "https://deno.land/std@0.78.0/log/mod.ts";

const consoleHandler = new ConsoleHandler(logLevels.trace);

const logger = new Logger(logLevels.trace, { handlers: [consoleHandler] });

logger.trace("Hello");
logger.debug("world");
logger.info(123456);
logger.warn(true);
logger.error({ foo: "bar", fizz: "bazz" });
```

Output

```sh
Hello
world
123456
true
{ foo: "bar", fizz: "bazz" }
```

## Advanced Usage

```ts
import {
  ConsoleHandler,
  FileHandler,
  Logger,
  logLevels,
} from "https://deno.land/std@0.78.0/log/mod.ts";

const consoleHandler = new ConsoleHandler(logLevels.info);
const fileHandler = new FileHandler(logLevels.warn, {
  filename: "./log.txt",
  formatter: ({ logLevel, message }) => `${logLevel.name} ${message}`,
});
const firstLogger = new Logger(logLevels.info, {
  handlers: [consoleHandler, fileHandler],
});

const secondLogger = new Logger(logLevels.error, {
  handlers: [consoleHandler],
});

firstLogger.debug("fizz"); // logs to no handler, because `consoleHandler` handler requires "info" level and `fileHandler` handler requires "warn" level
firstLogger.info("fizz"); // logs to `consoleHandler`, because `file` handler requires "warn" level
firstLogger.warn(41256); // logs to both `consoleHandler` and `fileHandler`

secondLogger.debug("fizz"); // logs to no handler, because this `secondLogger` has "Error" level
secondLogger.error({ productType: "book", value: "126.11" }); // logs to `consoleHandler`
```

## LogLevels

The default log levels are

| LogLevel | Name    | Code |
| -------: | ------- | ---- |
|    trace | "Trace" | 10   |
|    debug | "Debug" | 20   |
|     info | "Info"  | 30   |
|     warn | "Warn"  | 40   |
|    error | "Error" | 50   |

### Custom LogLevels

You can add custom logLevels by creating a `LogLevel` instance and adding it to
the handlers by calling `handler.addLogLevel`. Additionally, create a custom
`Logger` class and implement a method to handle the log level logic.

#### Example

```ts
import {
  ConsoleHandler,
  Logger,
  LogLevel,
  logLevels,
} from "https://deno.land/std@0.78.0/log/mod.ts";
import { bold, red } from "https://deno.land/std@0.78.0/fmt/colors.ts";

const customLogLevels = {
  ...logLevels,
  fatal: new LogLevel("Fatal", 60),
};

class CustomLogger extends Logger {
  fatal(message: unknown, ...args: unknown[]) {
    this.dispatch(customLogLevels.fatal, message, ...args);
    Deno.exit(1);
  }
}

const customConsoleHandler = new ConsoleHandler(customLogLevels.trace);
customConsoleHandler.addLogLevel(
  customLogLevels.fatal,
  (message: string) => console.log(bold(red(message))),
);

const customlogger = new CustomLogger(customLogLevels.trace, {
  handlers: [customConsoleHandler],
});

customlogger.trace("log trace message");
customlogger.debug("log debug message");
customlogger.info("log info message");
customlogger.warn("log warn message");
customlogger.error("log error message");
customlogger.fatal("log fatal message");
```

## Handler

A handler is responsible for actual output of log messages. When a handler is
called by a logger, it firstly checks that `LogRecord`'s level is not lower than
level of the handler. If level check passes, handlers formats log record into
string and outputs it to target.

### Custom Message Format

If you want to override default format of message you can define formatter
option for handler.

### Example

```ts
const consoleHandler = new ConsoleHandler(logLevels.debug, {
  formatter: ({ logLevel, message }) =>
    `Custom formatter: ${logLevel.name} ${message}`,
});
const logger = new Logger(logLevels.debug, {
  handlers: [consoleHandler],
});
logger.debug("Hello world");
```

Output

```sh
Custom formatter: Debug Hello world
```

## Built-in Handlers

This module comes with three built-in handlers:

### ConsoleHandler

This handler will output color coded log messages to the console.

| Method | Color   |
| -----: | ------- |
|  trace | default |
|  debug | default |
|   info | blue    |
|   warn | yellow  |
|  error | red     |

### FileHandler

This handler will output to a file.

#### Mode

You can specify a `mode` to change the behavior of the handler:

|                       Mode | Description                                                                                                                            |
| -------------------------: | -------------------------------------------------------------------------------------------------------------------------------------- |
| <nobr>"a" (default)</nobr> | Appends new log messages to the end of an existing log file, or create a new log file if none exists. The file will grow indefinitely. |
|                        "w" | Upon creation of the handler, any existing log file will be removed and a new one created.                                             |
|                        "x" | This will create a new log file and throw an error if one already exists.                                                              |

#### Permissions

This handler requires `--allow-write` permission on the log file.

### RotatingFileHandler

This handler extends the functionality of the `FileHandler` by _rotating_ the
log file when it reaches a certain size. `maxBytes` specifies the maximum size
in bytes that the log file can grow to before rolling over to a new one. If the
size of the new log message plus the current log file size exceeds `maxBytes`
then a roll over is triggered.

When a roll over occurs, before the log message is written, the log file is
renamed and appended with `.1`. If a `.1` version already existed, it would have
been renamed `.2` first and so on.

The maximum number of log files to keep is specified by `maxBackupCount`. After
the renames are complete the log message is written to the original, now blank,
file.

#### Mode

You can specify a `mode` to change the behavior of the handler:

|                       Mode | Description                                                                                                                                                      |
| -------------------------: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <nobr>"a" (default)</nobr> | Appends new log messages to the end of an existing log file, or create a new log file if none exists. The file will grow indefinitely.                           |
|                        "w" | In addition to starting with a clean log file, this mode will also cause any existing backups (up to `maxBackupCount`) to be deleted giving a fully clean slate. |
|                        "x" | This will create a new log file and throw an error if one or any backups (up to `maxBackupCount`) already exists.                                                |

#### Permissions

This handler requires both `--allow-read` and `--allow-write` permissions on the
log files.

### Custom message format

If you want to override default format of message you can define formatter
option for handler. It can be either simple string-based format that uses
LogRecord fields or more complicated function-based one that takes LogRecord as
argument and outputs string.

## Lazy Log Evaluation

Some log statements are expensive to compute. In these cases, you can use lazy
log evaluation to prevent the computation taking place if the logger won't log
the message. Methods `trace`, `debug`, `info`, `warn`, and `error` can therefore
also take a function as an argument.

### Example

```ts
const logger = new Logger(logLevels.error);

function expensiveFn() {
  const sum = 1 + 1;
  return `this is the sum: ${sum}`;
}

// expensiveFn is not being executed because logger has "error" level
logger.debug(expensiveFn);
// expensiveFn is being executed because logger has "error" level
logger.error(expensiveFn);
```
