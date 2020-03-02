import { FileOptions, isFileOptions } from "./_fs_common.ts";
import { OpenOptions } from "../../../cli/js/files.ts";
import { notImplemented } from "../_utils.ts";

export async function appendFile(path: string | number, data: string | Uint8Array, optionsOrCallback: string | FileOptions | Function, callback?: Function): Promise<void> {
  const callbackFn: Function | undefined = optionsOrCallback instanceof Function ? optionsOrCallback : callback;
  const options: string | FileOptions | undefined = optionsOrCallback instanceof Function ? undefined : optionsOrCallback;

  if (!callbackFn) {
    throw new Error('No callback function supplied');
  }

  try {
    validateEncoding(options);
    
    let rid: number;
    if (typeof path === "number") {
      rid = path;
    } else {
      const mode:number|undefined = isFileOptions(options) ? options.mode : undefined;
      const flag:string|undefined = isFileOptions(options) ? options.flag : undefined;
  
      if (mode) {
        notImplemented('Deno does not yet support setting mode on create');
      } 
      
      const file = await Deno.open(path, getOpenOptions(flag));
      rid = file.rid;
    }
  
    const buffer: Uint8Array = (data instanceof Uint8Array) ? data : new TextEncoder().encode(data);
  
    await Deno.write(rid, buffer);
    callbackFn();
  } catch (err) {
    callbackFn(err);
  } 
}

export function appendFileSync(path: string | number, data: string | Uint8Array, options?: string | FileOptions) {

}


function validateEncoding(encodingOption: string | FileOptions | undefined): void {
  if (!encodingOption) return;

  if (typeof encodingOption === "string") {
    if (encodingOption !== "utf8") {
      throw new Error("Only 'utf8' encoding is currently supported");
    }
  } else if (encodingOption.encoding && (encodingOption.encoding !== "utf8")) {
      throw new Error("Only 'utf8' encoding is currently supported");
  }
}

function getOpenOptions(flag: string|undefined): OpenOptions {
  if (!flag) {
    return {create:true, append: true};
  }

  let openOptions: OpenOptions;
  switch (flag) {
    case 'a' : {
      // 'a': Open file for appending. The file is created if it does not exist.
      openOptions = {create: true, append: true}
      break;
    }
    case 'ax' : {
      // 'ax': Like 'a' but fails if the path exists.
      openOptions = {createNew: true, write: true, append: true}
      break;
    }
    case 'a+' : {
      // 'a+': Open file for reading and appending. The file is created if it does not exist.
      openOptions = {read: true, create: true, append: true};
      break;
    }
    case 'ax+' : {
      // 'ax+': Like 'a+' but fails if the path exists.
      openOptions = {read: true, createNew: true, append: true};
      break;
    }
    case 'r' : {
      // 'r': Open file for reading. An exception occurs if the file does not exist.
      openOptions = {read: true};
      break;
    }
    case 'r+' : {
      // 'r+': Open file for reading and writing. An exception occurs if the file does not exist.
      openOptions = {read: true, write: true};
      break;
    }
    case 'w' : {
      // 'w': Open file for writing. The file is created (if it does not exist) or truncated (if it exists).
      openOptions = {create: true, write: true, truncate: true};
      break;
    }
    case 'wx' : {
      // 'wx': Like 'w' but fails if the path exists.
      openOptions = {createNew: true, write: true, truncate: true};
      break;
    }
    case 'w+' : {
      // 'w+': Open file for reading and writing. The file is created (if it does not exist) or truncated (if it exists).
      openOptions = {create: true, write: true, truncate: true, read: true};
      break;
    }
    case 'wx+' : {
      // 'wx+': Like 'w+' but fails if the path exists.
      openOptions = {createNew: true, write: true, truncate: true, read: true};
      break;
    }
    case 'as' : // 'as': Open file for appending in synchronous mode. The file is created if it does not exist.
    case 'as+' : // 'as+': Open file for reading and appending in synchronous mode. The file is created if it does not exist.
    case 'rs+' : {
      // 'rs+': Open file for reading and writing in synchronous mode. Instructs the operating system to bypass the local file system cache.
      throw new Error(`file system flag '${flag}' is not yet supported`);
    }
    default : {
      throw new Error(`Unrecognized file system flag: ${flag}`);
    }
  }

  return openOptions;
}
