const MAX_PATTERN_LENGTH = 1024 * 64
export const assertValidPattern: (pattern: any) => void = (
  pattern: any
): asserts pattern is string => {
  if (typeof pattern !== 'string') {
    throw new TypeError('invalid pattern')
  }

  if (pattern.length > MAX_PATTERN_LENGTH) {
    throw new TypeError('pattern is too long')
  }
}
