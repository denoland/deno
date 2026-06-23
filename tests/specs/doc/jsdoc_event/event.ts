/**
 * A connection that fires events when its state changes.
 *
 * @event open - Fired when the connection opens.
 * @event close - Fired when the connection closes.
 */
export class Conn extends EventTarget {
  /**
   * Open the connection.
   *
   * @fires Conn#open
   */
  open(): void {}

  /**
   * Close the connection.
   *
   * @emits Conn#close
   */
  close(): void {}

  /**
   * Subscribe to the upstream feed.
   *
   * @listens Conn#open
   */
  subscribe(): void {}
}
