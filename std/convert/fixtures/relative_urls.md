---
baseUrl: "http://example.com/base/"
---
# Absolutization of RFC 3986 URIs

## Absolute URI
[![section 4.3](http://example.com/logo)](http://example.com/)

## Network-path reference
[![section 4.2](//example.com/logo)](//example.com/)

## Absolute path
[![section 4.2](/path/to/img)](/path/to/content)

## Relative path
[![section 4.2](img)](content)

## Dot-relative path
[![section 3.3](./img)](./content)

[![section 3.3](../img)](../content)

## Same-document query
[![section 4.4](?type=image)](?)

## Same-document fragment
[![section 4.4](#img)](#)

## Empty
[section 4.2]()
