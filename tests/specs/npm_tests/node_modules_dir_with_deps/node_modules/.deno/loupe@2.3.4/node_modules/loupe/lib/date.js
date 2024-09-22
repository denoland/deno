import { truncate } from './helpers'

export default function inspectDate(dateObject, options) {
  // If we need to - truncate the time portion, but never the date
  const split = dateObject.toJSON().split('T')
  const date = split[0]
  return options.stylize(`${date}T${truncate(split[1], options.truncate - date.length - 1)}`, 'date')
}
