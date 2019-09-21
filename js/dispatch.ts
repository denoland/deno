/* eslint-disable @typescript-eslint/no-explicit-any */
export const dispatch: Record<string, any> = {};

export class DispatchOp {
  public name: string;
  public opId!: number;

  constructor(name: string) {
    // @ts-ignore
    // Deno.core.print("registering op " + name + "\n", true);
    // throw new Error(`Registering op: ${name}`);
    if (typeof dispatch[name] !== "undefined") {
      throw new Error(`Duplicate op: ${name}`);
    }

    this.name = name;
    this.opId = 0;
    dispatch[name] = this;
  }

  setOpId(opId: number): void {
    this.opId = opId;
  }

  static handleAsyncMsgFromRust(_opId: number, _buf: Uint8Array): any {
    throw new Error("Unimplemented");
  }

  static sendSync(
    _opId: number,
    _control: Uint8Array,
    _zeroCopy?: Uint8Array
  ): any {
    throw new Error("Unimplemented");
  }

  static sendAsync(
    _opId: number,
    _control: Uint8Array,
    _zeroCopy?: Uint8Array
  ): Promise<any> {
    throw new Error("Unimplemented");
  }
}
