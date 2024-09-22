import { inspectList } from './helpers'

export default function inspectArguments(args, options) {
  if (args.length === 0) return 'Arguments[]'
  options.truncate -= 13
  return `Arguments[ ${inspectList(args, options)} ]`
}
