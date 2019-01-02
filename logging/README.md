# Basic usage

```ts
import * as log from "https://deno.land/x/std/logging/index.ts";

// simple console logger
log.debug("Hello world");
log.info("Hello world");
log.warning("Hello world");
log.error("Hello world");
log.critical("500 Internal server error");

// configure as needed
await log.setup({
    handlers: {
        console: new log.handlers.ConsoleHandler("DEBUG"),
        file: new log.handlers.FileHandler("WARNING", "./log.txt"),
    },

    loggers: {
        default: {
            level: "DEBUG",
            handlers: ["console", "file"],
        }
    }
});

// get configured logger
const logger = log.getLogger("default");
logger.debug("fizz") // <- logs to `console`, because `file` handler requires 'WARNING' level
logger.warning("buzz") // <- logs to both `console` and `file` handlers

// if you try to use a logger that hasn't been configured
// you're good to go, it gets created automatically with level set to 0
// so no message is logged
const unknownLogger = log.getLogger("mystery");
unknownLogger.info("foobar") // no-op
```