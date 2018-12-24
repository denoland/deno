import { BaseHandler } from "../handler.ts";
import { LogLevel } from "../levels.ts";

export class ConsoleHandler extends BaseHandler {
  _log(level, ...args) {
    switch (level) {
      case LogLevel.DEBUG:
        console.log(...args);
        return;
      case LogLevel.INFO:
        console.info(...args);
        return;
      case LogLevel.WARNING:
        console.warn(...args);
        return;
      case LogLevel.ERROR:
        console.error(...args);
        return;
      case LogLevel.CRITICAL:
        console.error(...args);
        return;
      default:
        return;
    }
  }
}
