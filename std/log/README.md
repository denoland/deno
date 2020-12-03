# Log

## Usage

```ts
import * as log from "https://deno.land/std@$STD_VERSION/log/mod.ts";

// Simple default logger out of the box. You can customize it
// by overriding logger and handler named "default", or providing
// additional logger configurations. You can log any data type.
log.debug("Hello world");
log.info(123456);
log.warning(true);
log.error({ foo: "bar", fizz: "bazz" });
log.critical("500 Internal server error");

// custom configuration with 2 loggers (the default and `tasks` loggers).
await log.setup({
  handlers: {
    console: new log.handlers.ConsoleHandler("DEBUG"),

    file: new log.handlers.FileHandler("WARNING", {
      filename: "./log.txt",
      // you can change format of output message using any keys in `LogRecord`.
      formatter: "{levelName} {msg}",
    }),
  },

  loggers: {
    // configure default logger available via short-hand methods above.
    default: {
      level: "DEBUG",
      handlers: ["console", "file"],
    },

    tasks: {
      level: "ERROR",
      handlers: ["console"],
    },
  },
});

let logger;

// get default logger.
logger = log.getLogger();
logger.debug("fizz"); // logs to `console`, because `file` handler requires "WARNING" level.
logger.warning(41256); // logs to both `console` and `file` handlers.

// get custom logger
logger = log.getLogger("tasks");
logger.debug("fizz"); // won't get output because this logger has "ERROR" level.
logger.error({ productType: "book", value: "126.11" }); // log to `console`.

// if you try to use a logger that hasn't been configured
// you're good to go, it gets created automatically with level set to 0
// so no message is logged.
const unknownLogger = log.getLogger("mystery");
unknownLogger.info("foobar"); // no-op
```

## Advanced usage

### Loggers

Loggers are objects that you interact with. When you use a logger method it
constructs a `LogRecord` and passes it down to its handlers for output. To
create custom loggers, specify them in `loggers` when calling `log.setup`.

#### `LogRecord`

`LogRecord` is an object that encapsulates provided message and arguments as
well some meta data that can be later used when formatting a message.

```ts
class LogRecord {
  readonly msg: string;
  readonly args: any[];
  readonly date: Date;
  readonly level: number;
  readonly levelName: string;
  readonly loggerName: string;
}
```

### Handlers

Handlers are responsible for actual output of log messages. When a handler is
called by a logger, it firstly checks that `LogRecord`'s level is not lower than
level of the handler. If level check passes, handlers formats log record into
string and outputs it to target.

`log` module comes with three built-in handlers:

#### `ConsoleHandler`

This is the default logger. It will output color coded log messages to the
console via `console.log()`. This logger takes `HandlerOptions`:

```typescript
type FormatterFunction = (logRecord: LogRecord) => string;

interface HandlerOptions {
  formatter?: string | FormatterFunction; //see `Custom message format` below
}
```

#### `FileHandler`

This handler will output to a file using an optional mode (default is `a`, e.g.
append). The file will grow indefinitely. It uses a buffer for writing to file.
Logs can be manually flushed with `fileHandler.flush()`. Log messages with a log
level greater than error are immediately flushed. Logs are also flushed on
process completion. This logger takes `FileOptions`:

```typescript
interface FileHandlerOptions {
  formatter?: string | FormatterFunction; //see `Custom message format` below
  filename: string;
  mode?: LogMode; // 'a', 'w', 'x'
}
```

Behavior of the log modes is as follows:

- `'a'` - Default mode. Appends new log messages to the end of an existing log
  file, or create a new log file if none exists.
- `'w'` - Upon creation of the handler, any existing log file will be removed
  and a new one created.
- `'x'` - This will create a new log file and throw an error if one already
  exists.

This handler requires `--allow-write` permission on the log file.

#### `RotatingFileHandler`

This handler extends the functionality of the `FileHandler` by "rotating" the
log file when it reaches a certain size. `maxBytes` specifies the maximum size
in bytes that the log file can grow to before rolling over to a new one. If the
size of the new log message plus the current log file size exceeds `maxBytes`
then a roll over is triggered. When a roll over occurs, before the log message
is written, the log file is renamed and appended with `.1`. If a `.1` version
already existed, it would have been renamed `.2` first and so on. The maximum
number of log files to keep is specified by `maxBackupCount`. After the renames
are complete the log message is written to the original, now blank, file.

Example: Given `log.txt`, `log.txt.1`, `log.txt.2` and `log.txt.3`, a
`maxBackupCount` of 3 and a new log message which would cause `log.txt` to
exceed `maxBytes`, then `log.txt.2` would be renamed to `log.txt.3` (thereby
discarding the original contents of `log.txt.3` since 3 is the maximum number of
backups to keep), `log.txt.1` would be renamed to `log.txt.2`, `log.txt` would
be renamed to `log.txt.1` and finally `log.txt` would be created from scratch
where the new log message would be written.

This handler uses a buffer for writing log messages to file. Logs can be
manually flushed with `fileHandler.flush()`. Log messages with a log level
greater than ERROR are immediately flushed. Logs are also flushed on process
completion.

Options for this handler are:

```typescript
interface RotatingFileHandlerOptions {
  maxBytes: number;
  maxBackupCount: number;
  formatter?: string | FormatterFunction; //see `Custom message format` below
  filename: string;
  mode?: LogMode; // 'a', 'w', 'x'
}
```

Additional notes on `mode` as described above:

- `'a'` Default mode. As above, this will pick up where the logs left off in
  rotation, or create a new log file if it doesn't exist.
- `'w'` in addition to starting with a clean `filename`, this mode will also
  cause any existing backups (up to `maxBackupCount`) to be deleted on setup
  giving a fully clean slate.
- `'x'` requires that neither `filename`, nor any backups (up to
  `maxBackupCount`), exist before setup.

This handler requires both `--allow-read` and `--allow-write` permissions on the
log files.

### Custom message format

If you want to override default format of message you can define `formatter`
option for handler. It can be either simple string-based format that uses
`LogRecord` fields or more complicated function-based one that takes `LogRecord`
as argument and outputs string.

Eg.

```ts
await log.setup({
  handlers: {
    stringFmt: new log.handlers.ConsoleHandler("DEBUG", {
      formatter: "[{levelName}] {msg}"
    }),

    functionFmt: new log.handlers.ConsoleHandler("DEBUG", {
      formatter: logRecord => {
        let msg = `${logRecord.level} ${logRecord.msg}`;

        logRecord.args.forEach((arg, index) => {
          msg += `, arg${index}: ${arg}`;
        });

        return msg;
      }
    }),

    anotherFmt: new log.handlers.ConsoleHandler("DEBUG", {
      formatter: "[{loggerName}] - {levelName} {msg}"
    }),
  },

  loggers: {
     default: {
       level: "DEBUG",
       handlers: ["stringFmt", "functionFmt"],
     },
     dataLogger: {
       level: "INFO",
       handlers: ["anotherFmt"],
     }
  }
})

// calling:
log.debug("Hello, world!", 1, "two", [3, 4, 5]);
// results in:
[DEBUG] Hello, world! // output from "stringFmt" handler.
10 Hello, world!, arg0: 1, arg1: two, arg3: [3, 4, 5] // output from "functionFmt" formatter.

// calling:
log.getLogger("dataLogger").error("oh no!");
// results in:
[dataLogger] - ERROR oh no! // output from anotherFmt handler.
```

#### Custom handlers

Custom handlers can be implemented by subclassing `BaseHandler` or
`WriterHandler`.

`BaseHandler` is bare-bones handler that has no output logic at all,

`WriterHandler` is an abstract class that supports any target with `Writer`
interface.

During setup async hooks `setup` and `destroy` are called, you can use them to
open and close file/HTTP connection or any other action you might need.

For examples check source code of `FileHandler` and `TestHandler`.

### Inline Logging

Log functions return the data passed in the `msg` parameter. Data is returned
regardless if the logger actually logs it.

```ts
const stringData: string = logger.debug("hello world");
const booleanData: boolean = logger.debug(true, 1, "abc");
const fn = (): number => {
  return 123;
};
const resolvedFunctionData: number = logger.debug(fn());
console.log(stringData); // 'hello world'
console.log(booleanData); // true
console.log(resolvedFunctionData); // 123
```

### Lazy Log Evaluation

Some log statements are expensive to compute. In these cases, you can use lazy
log evaluation to prevent the computation taking place if the logger won't log
the message.

```ts
// `expensiveFn(5)` is only evaluated if this logger is configured for debug logging.
logger.debug(() => `this is expensive: ${expensiveFn(5)}`);
```

> NOTE: When using lazy log evaluation, `undefined` will be returned if the
> resolver function is not called because the logger won't log it. It is an
> antipattern use lazy evaluation with inline logging because the return value
> depends on the current log level.

Example:

```ts
await log.setup({
  handlers: {
    console: new log.handlers.ConsoleHandler("DEBUG"),
  },

  loggers: {
    tasks: {
      level: "ERROR",
      handlers: ["console"],
    },
  },
});

// not logged, as debug < error.
const data: string | undefined = logger.debug(() => someExpenseFn(5, true));
console.log(data); // undefined
```
