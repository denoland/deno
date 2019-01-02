import { open, File, Writer } from "deno";
import { getLevelByName } from "./levels.ts";
import { LogRecord } from "./logger.ts";

export class BaseHandler {
  level: number;
  levelName: string;

  constructor(levelName: string) {
    this.level = getLevelByName(levelName);
    this.levelName = levelName;
  }

  handle(logRecord: LogRecord) {
    if (this.level > logRecord.level) return;

    // TODO: implement formatter
    const msg = `${logRecord.levelName} ${logRecord.msg}`;

    return this.log(msg);
  }

  log(msg: string) { }
  async setup() { }
  async destroy() { }
}


export class ConsoleHandler extends BaseHandler {
  log(msg: string) {
    console.log(msg);
  }
}


export abstract class WriterHandler extends BaseHandler {
  protected _writer: Writer;

  log(msg: string) {
    const encoder = new TextEncoder();
    // promise is intentionally not awaited
    this._writer.write(encoder.encode(msg + "\n"));
  }
}


export class FileHandler extends WriterHandler {
  private _file: File;
  private _filename: string;

  constructor(levelName: string, filename: string) {
    super(levelName);
    this._filename = filename;
  }

  async setup() {
    // open file in append mode - write only
    this._file = await open(this._filename, 'a');
    this._writer = this._file;
  }

  async destroy() {
    await this._file.close();
  }
}