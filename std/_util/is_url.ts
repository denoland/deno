// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** isUrl checks if a url is valid */
export function isURl(url: string){
  try {
    new URL(url)
    return true
  }  catch {
    return false
  }
 }
