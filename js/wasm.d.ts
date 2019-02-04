// MIT License
// Copied from https://github.com/DefinitelyTyped/DefinitelyTyped/blob/master/types/webassembly-js-api/index.d.ts
// Type definitions for WebAssembly v1 (MVP)
// Project: https://github.com/winksaville/test-webassembly-js-ts
// Definitions by: 01alchemist <https://twitter.com/01alchemist>
//                 Wink Saville <wink@saville.com>
//                 Periklis Tsirakidis <https://github.com/periklis>
//                 Sergey Rubanov <https://github.com/chicoxyzzy>
// Definitions: https://github.com/DefinitelyTyped/DefinitelyTyped
// TypeScript Version: 2.2

/**
 * The WebAssembly namespace, see [WebAssembly](https://github.com/webassembly)
 * and [WebAssembly JS API](http://webassembly.org/getting-started/js-api/)
 * for more information.
 */
declare namespace WebAssembly {
  type Imports =  Array<{
      name: string;
      kind: string;
  }>;

  type Exports = Array<{
      module: string;
      name: string;
      kind: string;
  }>;

  type BufferSource = ArrayBufferView | ArrayBuffer;

  /**
   * WebAssembly.Module
   */
  class Module {
      constructor(bufferSource: BufferSource);
      static customSections(module: Module, sectionName: string): ArrayBuffer[];
      static exports(module: Module): Imports;
      static imports(module: Module): Exports;
  }

  /**
   * WebAssembly.Instance
   */
  class Instance {
      readonly exports: any;
      constructor(module: Module, importObject?: any);
  }

  /**
   * WebAssembly.Memory
   * Note: A WebAssembly page has a constant size of 65,536 bytes, i.e., 64KiB.
   */
  interface MemoryDescriptor {
      initial: number;
      maximum?: number;
  }

  class Memory {
      readonly buffer: ArrayBuffer;
      constructor(memoryDescriptor: MemoryDescriptor);
      grow(numPages: number): number;
  }

  /**
   * WebAssembly.Table
   */
  interface TableDescriptor {
      element: "anyfunc";
      initial: number;
      maximum?: number;
  }

  class Table {
      readonly length: number;
      constructor(tableDescriptor: TableDescriptor);
      get(index: number): (args: any[]) => any;
      grow(numElements: number): number;
      set(index: number, value: (args: any[]) => any): void;
  }

  /**
   * Errors
   */
  class CompileError extends Error {
      readonly fileName: string;
      readonly lineNumber: string;
      readonly columnNumber: string;
      constructor(message?: string, fileName?: string, lineNumber?: number);
      toString(): string;
  }

  class LinkError extends Error {
      readonly fileName: string;
      readonly lineNumber: string;
      readonly columnNumber: string;
      constructor(message?: string, fileName?: string, lineNumber?: number);
      toString(): string;
  }

  class RuntimeError extends Error {
      readonly fileName: string;
      readonly lineNumber: string;
      readonly columnNumber: string;
      constructor(message?: string, fileName?: string, lineNumber?: number);
      toString(): string;
  }

  function compile(bufferSource: BufferSource): Promise<Module>;

  interface ResultObject {
      module: Module;
      instance: Instance;
  }

  function instantiate(bufferSource: BufferSource, importObject?: object): Promise<ResultObject>;
  function instantiate(module: Module, importObject?: object): Promise<Instance>;

  function validate(bufferSource: BufferSource): boolean;
}
