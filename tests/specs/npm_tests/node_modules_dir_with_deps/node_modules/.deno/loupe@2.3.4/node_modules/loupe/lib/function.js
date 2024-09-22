import getFunctionName from 'get-func-name'
import { truncate } from './helpers'

export default function inspectFunction(func, options) {
  const name = getFunctionName(func)
  if (!name) {
    return options.stylize('[Function]', 'special')
  }
  return options.stylize(`[Function ${truncate(name, options.truncate - 11)}]`, 'special')
}
