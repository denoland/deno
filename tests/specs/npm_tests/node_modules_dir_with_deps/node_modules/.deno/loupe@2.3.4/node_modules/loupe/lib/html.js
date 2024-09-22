import { truncator, inspectList } from './helpers'

export function inspectAttribute([key, value], options) {
  options.truncate -= 3
  if (!value) {
    return `${options.stylize(key, 'yellow')}`
  }
  return `${options.stylize(key, 'yellow')}=${options.stylize(`"${value}"`, 'string')}`
}

export function inspectHTMLCollection(collection, options) {
  // eslint-disable-next-line no-use-before-define
  return inspectList(collection, options, inspectHTML, '\n')
}

export default function inspectHTML(element, options) {
  const properties = element.getAttributeNames()
  const name = element.tagName.toLowerCase()
  const head = options.stylize(`<${name}`, 'special')
  const headClose = options.stylize(`>`, 'special')
  const tail = options.stylize(`</${name}>`, 'special')
  options.truncate -= name.length * 2 + 5
  let propertyContents = ''
  if (properties.length > 0) {
    propertyContents += ' '
    propertyContents += inspectList(
      properties.map(key => [key, element.getAttribute(key)]),
      options,
      inspectAttribute,
      ' '
    )
  }
  options.truncate -= propertyContents.length
  const truncate = options.truncate
  let children = inspectHTMLCollection(element.children, options)
  if (children && children.length > truncate) {
    children = `${truncator}(${element.children.length})`
  }
  return `${head}${propertyContents}${headClose}${children}${tail}`
}
