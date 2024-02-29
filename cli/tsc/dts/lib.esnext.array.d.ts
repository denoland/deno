/*! *****************************************************************************
Copyright (c) Microsoft Corporation. All rights reserved.
Licensed under the Apache License, Version 2.0 (the "License"); you may not use
this file except in compliance with the License. You may obtain a copy of the
License at http://www.apache.org/licenses/LICENSE-2.0

THIS CODE IS PROVIDED ON AN *AS IS* BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
KIND, EITHER EXPRESS OR IMPLIED, INCLUDING WITHOUT LIMITATION ANY IMPLIED
WARRANTIES OR CONDITIONS OF TITLE, FITNESS FOR A PARTICULAR PURPOSE,
MERCHANTABLITY OR NON-INFRINGEMENT.

See the Apache Version 2.0 License for specific language governing permissions
and limitations under the License.
***************************************************************************** */

/// <reference no-default-lib="true"/>

// NOTE(bartlomieju): taken from https://github.com/microsoft/TypeScript/issues/50803#issuecomment-1249030430
// while we wait for these types to officially ship
interface ArrayConstructor {
  fromAsync<T>(
      iterableOrArrayLike: AsyncIterable<T> | Iterable<T | Promise<T>> | ArrayLike<T | Promise<T>>,
  ): Promise<T[]>;
  
  fromAsync<T, U>(
      iterableOrArrayLike: AsyncIterable<T> | Iterable<T> | ArrayLike<T>, 
      mapFn: (value: Awaited<T>) => U, 
      thisArg?: any,
  ): Promise<Awaited<U>[]>;
}
