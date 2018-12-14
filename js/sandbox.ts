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
    const [result, errMsg] = libdeno.runInContext(this.id, code);
    if (errMsg) {
      throw new Error(errMsg);
    }
    return result;
  }
}

// tslint:disable-next-line:no-any
export function sandbox(model: any): DenoSandbox {
  const [id, context] = libdeno.makeContext(model);
  for (const key in model) {
    context[key] = model[key];
  }

  return new DenoSandboxImpl(id, context);
}
