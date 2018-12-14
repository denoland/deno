import { libdeno } from "./libdeno";

export interface DenoSandbox {
  // tslint:disable-next-line:no-any
  context: any;
  // tslint:disable-next-line:no-any
  execute: (code: string) => any;
}

class DenoSandboxImpl implements DenoSandbox {
  constructor(public context: {}) {}
  execute(code: string) {
    const [result, errMsg] = libdeno.runInContext(this.context, code);
    if (errMsg) {
      throw new Error(errMsg);
    }
    return result;
  }
}

// tslint:disable-next-line:no-any
export function sandbox(model: any): DenoSandbox {
  const context = libdeno.makeContext();
  for (const key in model) {
    context[key] = model[key];
  }

  return new DenoSandboxImpl(context);
}
