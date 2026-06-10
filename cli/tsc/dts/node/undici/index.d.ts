import Dispatcher from './dispatcher.d.ts'
import { setGlobalDispatcher, getGlobalDispatcher } from './global-dispatcher.d.ts'
import { setGlobalOrigin, getGlobalOrigin } from './global-origin.d.ts'
import Pool from './pool.d.ts'
import { RedirectHandler, DecoratorHandler } from './handlers.d.ts'

import BalancedPool from './balanced-pool.d.ts'
import Client from './client.d.ts'
import H2CClient from './h2c-client.d.ts'
import buildConnector from './connector.d.ts'
import errors from './errors.d.ts'
import Agent from './agent.d.ts'
import MockClient from './mock-client.d.ts'
import MockPool from './mock-pool.d.ts'
import MockAgent from './mock-agent.d.ts'
import { MockCallHistory, MockCallHistoryLog } from './mock-call-history.d.ts'
import mockErrors from './mock-errors.d.ts'
import ProxyAgent from './proxy-agent.d.ts'
import EnvHttpProxyAgent from './env-http-proxy-agent.d.ts'
import RetryHandler from './retry-handler.d.ts'
import RetryAgent from './retry-agent.d.ts'
import { request, pipeline, stream, connect, upgrade } from './api.d.ts'
import interceptors from './interceptors.d.ts'

export * from './util.d.ts'
export * from './cookies.d.ts'
export * from './eventsource.d.ts'
export * from './fetch.d.ts'
export * from './formdata.d.ts'
export * from './diagnostics-channel.d.ts'
export * from './websocket.d.ts'
export * from './content-type.d.ts'
export * from './cache.d.ts'
export { Interceptable } from './mock-interceptor.d.ts'

export { Dispatcher, BalancedPool, Pool, Client, buildConnector, errors, Agent, request, stream, pipeline, connect, upgrade, setGlobalDispatcher, getGlobalDispatcher, setGlobalOrigin, getGlobalOrigin, interceptors, MockClient, MockPool, MockAgent, MockCallHistory, MockCallHistoryLog, mockErrors, ProxyAgent, EnvHttpProxyAgent, RedirectHandler, DecoratorHandler, RetryHandler, RetryAgent, H2CClient }
export default Undici

declare namespace Undici {
  const Dispatcher: typeof import('./dispatcher.d.ts').default
  const Pool: typeof import('./pool.d.ts').default
  const RedirectHandler: typeof import ('./handlers.d.ts').RedirectHandler
  const DecoratorHandler: typeof import ('./handlers.d.ts').DecoratorHandler
  const RetryHandler: typeof import ('./retry-handler.d.ts').default
  const BalancedPool: typeof import('./balanced-pool.d.ts').default
  const Client: typeof import('./client.d.ts').default
  const H2CClient: typeof import('./h2c-client.d.ts').default
  const buildConnector: typeof import('./connector.d.ts').default
  const errors: typeof import('./errors.d.ts').default
  const Agent: typeof import('./agent.d.ts').default
  const setGlobalDispatcher: typeof import('./global-dispatcher.d.ts').setGlobalDispatcher
  const getGlobalDispatcher: typeof import('./global-dispatcher.d.ts').getGlobalDispatcher
  const request: typeof import('./api.d.ts').request
  const stream: typeof import('./api.d.ts').stream
  const pipeline: typeof import('./api.d.ts').pipeline
  const connect: typeof import('./api.d.ts').connect
  const upgrade: typeof import('./api.d.ts').upgrade
  const MockClient: typeof import('./mock-client.d.ts').default
  const MockPool: typeof import('./mock-pool.d.ts').default
  const MockAgent: typeof import('./mock-agent.d.ts').default
  const MockCallHistory: typeof import('./mock-call-history.d.ts').MockCallHistory
  const MockCallHistoryLog: typeof import('./mock-call-history.d.ts').MockCallHistoryLog
  const mockErrors: typeof import('./mock-errors.d.ts').default
  const fetch: typeof import('./fetch.d.ts').fetch
  const Headers: typeof import('./fetch.d.ts').Headers
  const Response: typeof import('./fetch.d.ts').Response
  const Request: typeof import('./fetch.d.ts').Request
  const FormData: typeof import('./formdata.d.ts').FormData
  const caches: typeof import('./cache.d.ts').caches
  const interceptors: typeof import('./interceptors.d.ts').default
  const cacheStores: {
    MemoryCacheStore: typeof import('./cache-interceptor.d.ts').default.MemoryCacheStore,
    SqliteCacheStore: typeof import('./cache-interceptor.d.ts').default.SqliteCacheStore
  }
}
