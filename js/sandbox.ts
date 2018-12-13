import { libdeno } from "./libdeno";

export interface DenoSandbox {
  // tslint:disable-next-line:no-any
  context: any;
  // tslint:disable-next-line:no-any
  execute: (code: string) => any;
}

class DenoSandboxImpl implements DenoSandbox {
  constructor(private id: number, public context: {}) {}
  execute(code: string) {
    return libdeno.runInContext(this.id, code);
  }
}

export function sandbox(): DenoSandbox {
  const [id, context] = libdeno.makeContext({});
  return new DenoSandboxImpl(id, context);
}
