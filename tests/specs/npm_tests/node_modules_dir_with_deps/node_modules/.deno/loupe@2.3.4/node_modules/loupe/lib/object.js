import { inspectProperty, inspectList } from './helpers'

export default function inspectObject(object, options) {
  const properties = Object.getOwnPropertyNames(object)
  const symbols = Object.getOwnPropertySymbols ? Object.getOwnPropertySymbols(object) : []
  if (properties.length === 0 && symbols.length === 0) {
    return '{}'
  }
  options.truncate -= 4
  options.seen = options.seen || []
  if (options.seen.indexOf(object) >= 0) {
    return '[Circular]'
  }
  options.seen.push(object)
  const propertyContents = inspectList(
    properties.map(key => [key, object[key]]),
    options,
    inspectProperty
  )
  const symbolContents = inspectList(
    symbols.map(key => [key, object[key]]),
    options,
    inspectProperty
  )
  options.seen.pop()
  let sep = ''
  if (propertyContents && symbolContents) {
    sep = ', '
  }
  return `{ ${propertyContents}${sep}${symbolContents} }`
}
