import getFuncName from 'get-func-name'
import inspectObject from './object'

const toStringTag = typeof Symbol !== 'undefined' && Symbol.toStringTag ? Symbol.toStringTag : false

export default function inspectClass(value, options) {
  let name = ''
  if (toStringTag && toStringTag in value) {
    name = value[toStringTag]
  }
  name = name || getFuncName(value.constructor)
  // Babel transforms anonymous classes to the name `_class`
  if (!name || name === '_class') {
    name = '<Anonymous Class>'
  }
  options.truncate -= name.length
  return `${name}${inspectObject(value, options)}`
}
