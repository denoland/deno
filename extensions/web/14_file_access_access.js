import { join, basename } from 'https://deno.land/std@0.98.0/path/mod.ts'

// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { } = window.__bootstrap.io;
  const { } = window.__bootstrap.fs;
  const { } = window.__bootstrap.util;
  const { BlobReference } = window.__bootstrap.file;

  const illegalConstructorKey = Symbol("illegalConstructorKey");
  const Deno = globalThis.Deno

  const errors = {
    INVALID: ['seeking position failed.', 'InvalidStateError'],
    GONE: ['A requested file or directory could not be found at the time an operation was processed.', 'NotFoundError'],
    MISMATCH: ['The path supplied exists, but was not an entry of requested type.', 'TypeMismatchError'],
    MOD_ERR: ['The object can not be modified in this way.', 'InvalidModificationError'],
    SYNTAX: m => [`Failed to execute 'write' on 'UnderlyingSinkBase': Invalid params passed. ${m}`, 'SyntaxError'],
    SECURITY: ['It was determined that certain files are unsafe for access within a Web application, or that too many calls are being made on file resources.', 'SecurityError'],
    DISALLOWED: ['The request is not allowed by the user agent or the platform in the current context.', 'NotAllowedError']
  }

  const { INVALID, GONE, MISMATCH, MOD_ERR, SYNTAX, DISALLOWED } = errors


  // TODO: Get a file backed up by fs, aka: BlobReferences.fromPath()
  /** @param {string} path */
  async function fromPath (path) {
    // BlobReference.from(path)
    const e = Deno.readFileSync(path)
    const s = await Deno.stat(path)
    return new File([e], basename(path), { lastModified: Number(s.mtime) })
  }

  class Sink {
    /**
     * @param {Deno.File} fileHandle
     * @param {number} size
     */
    constructor (fileHandle, size) {
      this.fileHandle = fileHandle
      this.size = size
      this.position = 0
    }
    async abort() {
      await this.fileHandle.close()
    }
    async write (chunk) {
      if (typeof chunk === 'object') {
        if (chunk.type === 'write') {
          if (Number.isInteger(chunk.position) && chunk.position >= 0) {
            this.position = chunk.position
          }
          if (!('data' in chunk)) {
            await this.fileHandle.close()
            throw new DOMException(...SYNTAX('write requires a data argument'))
          }
          chunk = chunk.data
        } else if (chunk.type === 'seek') {
          if (Number.isInteger(chunk.position) && chunk.position >= 0) {
            if (this.size < chunk.position) {
              throw new DOMException(...INVALID)
            }
            this.position = chunk.position
            return
          } else {
            await this.fileHandle.close()
            throw new DOMException(...SYNTAX('seek requires a position argument'))
          }
        } else if (chunk.type === 'truncate') {
          if (Number.isInteger(chunk.size) && chunk.size >= 0) {
            await this.fileHandle.truncate(chunk.size)
            this.size = chunk.size
            if (this.position > this.size) {
              this.position = this.size
            }
            return
          } else {
            await this.fileHandle.close()
            throw new DOMException(...SYNTAX('truncate requires a size argument'))
          }
        }
      }

      if (chunk instanceof ArrayBuffer) {
        chunk = new Uint8Array(chunk)
      } else if (typeof chunk === 'string') {
        chunk = new TextEncoder().encode(chunk)
      } else if (chunk instanceof Blob) {
        await this.fileHandle.seek(this.position, Deno.SeekMode.Start)
        for await (const data of chunk.stream()) {
          const written = await this.fileHandle.write(data)
          this.position += written
          this.size += written
        }
        return
      }
      await this.fileHandle.seek(this.position, Deno.SeekMode.Start)
      const written = await this.fileHandle.write(chunk)
      this.position += written
      this.size += written
    }

    async close () {
      await this.fileHandle.close()
    }
  }


  /*--------------------------------------------------------------------------
    pickers (showSaveFilePicker, showOpenFilePicker, showDirectoryPicker)
  ---------------------------------------------------------------------------*/

  /**
   * @param {Object} [options]
   * @param {boolean} [options.excludeAcceptAllOption=false] Prevent user for selecting any
   * @param {Object[]} [options.accepts] Files you want to accept
   * @param {string} [options.suggestedName] The filename suggested when saving
   * @returns Promise<FileSystemDirectoryHandle>
   */
  async function showSaveFilePicker (options = {}) {
    throw new Error('Not implemented, Lack of UI')
  }

  /**
   * @param {Object} [options]
   * @param {boolean} [options.multiple] If you want to allow more than one file
   * @param {boolean} [options.excludeAcceptAllOption=false] Prevent user for selecting any file
   * @param {Object[]} [options.accepts] Files you want to accept
   * @returns Promise<FileSystemDirectoryHandle>
   */
  async function showOpenFilePicker (options = {}) {
    throw new Error('Not implemented, Lack of UI')
  }

  /**
   * @returns Promise<FileSystemDirectoryHandle>
   */
  async function showDirectoryPicker () {
    throw new Error('Not implemented, Lack of UI')
  }


  /*--------------------------------------------------------------------------
    Getting a directory programmatically
    aka: (navigator.storage.getDirectory)
  ---------------------------------------------------------------------------*/

  async function getDirectory () {
    const handle = new FileSystemDirectoryHandle(illegalConstructorKey)
    await handle.requestPermission({mode: 'read', path: '.'}).catch(e => {
      throw new DOMException(...DISALLOWED)
    })
    await handle.requestPermission({mode: 'write', path: '.'}).catch(e => {
      throw new DOMException(...DISALLOWED)
    })
    set(handle, { path: Deno.cwd() })
    return handle
  }


  /*--------------------------------------------------------------------------
    FileSystemWritableFileStream
  ---------------------------------------------------------------------------*/

  class FileSystemWritableFileStream extends WritableStream {
    #closed
    // @ts-ignore
    constructor (key, ...args) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super(...args)

      /** @private */
      this.#closed = false
    }
    close () {
      this.#closed = true
      const w = this.getWriter()
      const p = w.close()
      w.releaseLock()
      return p
    }

    /** @param {number} position */
    seek (position) {
      return this.write({ type: 'seek', position })
    }

    /** @param {number} size */
    truncate (size) {
      return this.write({ type: 'truncate', size })
    }

    write (data) {
      if (this.#closed) {
        return Promise.reject(new TypeError('Cannot write to a CLOSED writable stream'))
      }

      const writer = this.getWriter()
      const p = writer.write(data)
      writer.releaseLock()
      return p
    }

    get [Symbol.toStringTag]() {
      return "FileSystemWritableFileStream";
    }
  }


  /*--------------------------------------------------------------------------
    FileSystemWritableFileStream
  ---------------------------------------------------------------------------*/

  const wm = new WeakMap()
  const get = wm.get.bind(wm)
  const set = (x, y) => Object.assign(get(x), y)

  class FileSystemHandle {
    constructor (key) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      wm.set(this, { kind: 'directory', name: '', path: '' })
    }

    get name () {
      return wm.get(this).name
    }

    get kind () {
      return wm.get(this).kind
    }

    /**
     * @param {object} options
     * @param {('read'|'write')} options.mode
     */
    async queryPermission (options) {
      const status = await Deno.permissions.query({ name: options.mode, path: get(this).path })
      return status.state
    }

    async requestPermission (options = {}) {
      const status = await Deno.permissions.request({ name: options.mode, path: get(this).path })
      return status.state
    }

    /** @param {FileSystemHandle} other */
    async isSameEntry (other) {
      if (this === other) return true
      return get(this).path === get(other).path
    }

    get [Symbol.toStringTag]() {
      return "FileSystemHandle";
    }
  }


  /*--------------------------------------------------------------------------
    FileSystemFileHandle
  ---------------------------------------------------------------------------*/

  class FileSystemFileHandle extends FileSystemHandle {
    /**
     * @param  {Object} [options={}]
     * @param  {boolean} [options.keepExistingData]
     * @returns {Promise<FileSystemWritableFileStream>}
     */
    async createWritable (options = {}) {
      const path = get(this).path
      const fileHandle = await Deno.open(path, {write: true}).catch(err => {
        if (err.name === 'NotFound') throw new DOMException(...GONE)
        throw err
      })
      const { size } = await fileHandle.stat()
      const sink = new Sink(fileHandle, size)

      return new FileSystemWritableFileStream(
        illegalConstructorKey,
        sink
      )
    }

    async getFile () {
      const path = get(this).path
      await Deno.stat(path).catch(err => {
        if (err.name === 'NotFound') throw new DOMException(...GONE)
      })
      return fromPath(path)
    }

    get [Symbol.toStringTag]() {
      return "FileSystemFileHandle";
    }
  }


  /*--------------------------------------------------------------------------
    FileSystemDirectoryHandle
  ---------------------------------------------------------------------------*/

  class FileSystemDirectoryHandle extends FileSystemHandle {
    /**
     * @param {string} name Name of the directory
     * @param {object} [options]
     * @param {boolean} [options.create] create the directory if don't exist
     * @returns {Promise<FileSystemDirectoryHandle>}
     */
    async getDirectoryHandle (name, options = {}) {
      if (name === '') throw new TypeError(`Name can't be an empty string.`)
      if (name === '.' || name === '..' || name.includes('/')) throw new TypeError(`Name contains invalid characters.`)

      const path = join(get(this).path, name)
      const stat = await Deno.lstat(path).catch(err => {
        if (err.name !== 'NotFound') throw err
      })
      const isDirectory = stat?.isDirectory
      if (stat && isDirectory) {}
      else if (stat && !isDirectory) throw new DOMException(...MISMATCH)
      else if (!options.create) throw new DOMException(...GONE)
      else await Deno.mkdir(path)

      const handle = new FileSystemDirectoryHandle(illegalConstructorKey)
      set(handle, { path, name })
      return handle
    }

    /** @returns {AsyncGenerator<[string, FileSystemHandle], void, unknown>} */
    async * entries () {
      const dir = get(this).path
      try {
        for await (const dirEntry of Deno.readDir(dir)) {
          const { name } = dirEntry
          const path = join(dir, name)
          const stat = await Deno.lstat(path)
          let handle
          if (stat.isFile) {
            handle = new FileSystemFileHandle(illegalConstructorKey)
            set(handle, { path, name, kind: 'file' })
          } else if (stat.isDirectory) {
            handle = new FileSystemDirectoryHandle(illegalConstructorKey)
            set(handle, { path, name })
          }
          yield [name, handle]
        }
      } catch (err) {
        throw err.name === 'NotFound' ? new DOMException(...GONE) : err
      }
    }

    /**
     * @param {string} name Name of the file
     * @param {object} [options]
     * @param {boolean} [options.create] create the file if don't exist
     * @returns {Promise<FileSystemFileHandle>}
     */
    async getFileHandle (name, options = {}) {
      if (name === '') throw new TypeError(`Name can't be an empty string.`)
      if (name === '.' || name === '..' || name.includes('/')) throw new TypeError(`Name contains invalid characters.`)
      options.create = !!options.create

      const path = join(get(this).path, name)
      const stat = await Deno.lstat(path).catch(err => {
        if (err.name !== 'NotFound') throw err
      })

      const isFile = stat?.isFile
      if (stat && isFile) {}
      else if (stat && !isFile) throw new DOMException(...MISMATCH)
      else if (!options.create) throw new DOMException(...GONE)
      else {
        const c = await Deno.open(path, { create: true, write: true })
        c.close()
      }
      const handle = new FileSystemFileHandle(illegalConstructorKey)
      set(handle, { path, name, kind: 'file' })
      return handle
    }

    /**
     * @param {string} name
     * @param {object} [options]
     * @param {boolean} [options.recursive]
     */
    async removeEntry (name, options = {}) {
      if (name === '') throw new TypeError(`Name can't be an empty string.`)
      if (name === '.' || name === '..' || name.includes('/')) throw new TypeError(`Name contains invalid characters.`)
      options.recursive = !!options.recursive

      const path = join(get(this).path, name)
      const stat = await Deno.lstat(path).catch(err => {
        if (err.name === 'NotFound') throw new DOMException(...GONE)
        throw err
      })

      if (stat.isDirectory) {
        if (options.recursive) {
          await Deno.remove(path, { recursive: true }).catch(err => {
            if (err.code === 'ENOTEMPTY') throw new DOMException(...MOD_ERR)
            throw err
          })
        } else {
          await Deno.remove(path).catch(() => {
            throw new DOMException(...MOD_ERR)
          })
        }
      } else {
        await Deno.remove(path)
      }
    }

    [Symbol.asyncIterator]() {
      return this.entries()
    }

    get [Symbol.toStringTag]() {
      return "FileSystemDirectoryHandle";
    }
  }

  Object.assign(window.__bootstrap.file, {
    showSaveFilePicker,
    showOpenFilePicker,
    showDirectoryPicker,

    FileSystemWritableFileStream,
    FileSystemHandle,
    FileSystemFileHandle,
    FileSystemDirectoryHandle
  });
})(this);
