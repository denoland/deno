import Client from './client.d.ts'
import Dispatcher from './dispatcher.d.ts'
import MockAgent from './mock-agent.d.ts'
import { MockInterceptor, Interceptable } from './mock-interceptor.d.ts'

export default MockClient

/** MockClient extends the Client API and allows one to mock requests. */
declare class MockClient extends Client implements Interceptable {
  constructor (origin: string, options: MockClient.Options)
  /** Intercepts any matching requests that use the same origin as this mock client. */
  intercept (options: MockInterceptor.Options): MockInterceptor
  /** Dispatches a mocked request. */
  dispatch (options: Dispatcher.DispatchOptions, handlers: Dispatcher.DispatchHandler): boolean
  /** Closes the mock client and gracefully waits for enqueued requests to complete. */
  close (): Promise<void>
}

declare namespace MockClient {
  /** MockClient options. */
  export interface Options extends Client.Options {
    /** The agent to associate this MockClient with. */
    agent: MockAgent;
  }
}
