import { File } from "./files.ts";
import { sendSync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";

/** Check if a given resource is TTY. */
export function isatty(rid: number): boolean {
  return sendSync(dispatch.OP_ISATTY, { rid });
}

/** Extended file abstraction for TTY input */
export class TTYInput extends File {
  constructor(rid: number) {
    super(rid);
    if (rid !== 0 && !isatty(rid)) {
      throw new Error("Given resource is not a TTY");
    }
  }

  /** Is TTY under raw mode. */
  get isRaw(): boolean {
    return this._isRaw;
  }
  private _isRaw = false;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private _restoreInfo: { [key: string]: any } = {};

  /** Set TTY to be under raw mode. */
  setRaw(mode: boolean): void {
    if (this._isRaw === mode) {
      return;
    }
    if (mode) {
      this._restoreInfo = sendSync(dispatch.OP_SET_RAW, {
        rid: this.rid,
        raw: true
      });
      this._isRaw = true;
    } else {
      sendSync(dispatch.OP_SET_RAW, {
        rid: this.rid,
        raw: false,
        ...this._restoreInfo
      });
      this._isRaw = false;
    }
  }
}

/** An instance of `TTYInput` for stdin. */
export const stdin = new TTYInput(0);
/** An instance of `File` for stdout. */
export const stdout = new File(1);
/** An instance of `File` for stderr. */
export const stderr = new File(2);
