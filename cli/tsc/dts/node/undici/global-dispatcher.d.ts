import Dispatcher from './dispatcher.d.ts'

declare function setGlobalDispatcher<DispatcherImplementation extends Dispatcher> (dispatcher: DispatcherImplementation): void
declare function getGlobalDispatcher (): Dispatcher

export {
  getGlobalDispatcher,
  setGlobalDispatcher
}
