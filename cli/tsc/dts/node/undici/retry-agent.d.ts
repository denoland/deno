import Dispatcher from './dispatcher.d.ts'
import RetryHandler from './retry-handler.d.ts'

export default RetryAgent

declare class RetryAgent extends Dispatcher {
  constructor (dispatcher: Dispatcher, options?: RetryHandler.RetryOptions)
}
