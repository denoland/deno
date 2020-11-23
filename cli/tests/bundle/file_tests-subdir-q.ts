// deno-lint-ignore-file
export interface D {
  resolve: any;
  reject: any;
}

export function d(): D {
  let methods;
  const promise = new Promise((resolve, reject) => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods);
}
