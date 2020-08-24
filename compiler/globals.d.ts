import _ts from "../cli/dts/typescript.d.ts";

declare global {
  namespace ts {
    export = _ts;
  }

  namespace ts {
    // this are marked @internal in TypeScript, but we need to access them,
    // there is a risk these could change in future versions of TypeScript
    export const libs: string[];
    export const libMap: Map<string, string>;
    export const performance: {
      enable(): void;
      disable(): void;
      getDuration(value: string): number;
    };

    interface SourceFile {
      version?: string;
    }
  }

  namespace Deno {
    const core: {
      decode(value: Uint8Array): string;
      dispatch(opId: number, msg: Uint8Array): Uint8Array;
      encode(value: string): Uint8Array;
      ops(): Record<string, number>;
      print(msg: string): void;
    };
  }
}
