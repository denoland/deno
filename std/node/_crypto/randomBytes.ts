import { Buffer } from "../buffer.ts";

export const MAX_RANDOM_VALUES = 65536;
export const MAX_SIZE = 4294967295;

function generateRandomBytes(size: number) {
  if (size > MAX_SIZE) {
    throw new RangeError(
      `The value of "size" is out of range. It must be >= 0 && <= ${MAX_SIZE}. Received ${size}`,
    );
  }

  const bytes = Buffer.allocUnsafe(size);

  //Work around for getRandomValues max generation
  if (size > MAX_RANDOM_VALUES) {
    for (let generated = 0; generated < size; generated += MAX_RANDOM_VALUES) {
      crypto.getRandomValues(
        bytes.slice(generated, generated + MAX_RANDOM_VALUES),
      );
    }
  } else {
    crypto.getRandomValues(bytes);
  }

  return bytes;
}

/**
 * @param size Buffer length, must be equal or greater than zero
 */
export default function randomBytes(size: number): Buffer;
export default function randomBytes(
  size: number,
  cb?: (err: Error | null, buf?: Buffer) => void,
): void;
export default function randomBytes(
  size: number,
  cb?: (err: Error | null, buf?: Buffer) => void,
): Buffer | void {
  if (typeof cb === "function") {
    try {
      cb(null, generateRandomBytes(size));
    } catch (e) {
      //NodeJS nonsense
      //If the size is out of range it will throw sync, otherwise throw async
      if (
        e instanceof RangeError &&
        e.message.includes('The value of "size" is out of range')
      ) {
        throw e;
      } else {
        cb(e);
      }
    }
  } else {
    return generateRandomBytes(size);
  }
}
