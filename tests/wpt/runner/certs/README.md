# WPT Test Certificates

The web-platform-tests project maintains a set of SSL certificates to allow
contributors to execute tests requiring HTTPS locally.

## Trusting Root CA

To prevent browser SSL warnings when running HTTPS tests locally, the
web-platform-tests Root CA file `cacert.pem` in the `tools/certs/` directory
must be added as a trusted certificate in your OS/browser.

For Firefox, go to about:preferences and search for "certificates".

For browsers that use the Certificate Authorities of the underlying OS, such as
Chrome and Safari, you need to adjust the OS. For macOS, go to Keychain Access
and add the certificate under **login**.

**NOTE**: The CA should not be installed in any browser profile used
outside of tests, since it may be used to generate fake
certificates. For browsers that use the OS certificate store, tests
should therefore not be run manually outside a dedicated OS instance
(e.g. a VM). To avoid this problem when running tests in Chrome or
Firefox, use `wpt run`, which disables certificate checks and therefore
doesn't require the root CA to be trusted.

## Regenerating certificates

The easiest way to regenerate the pregenerated certificates is to the
the command

```
wpt regen-certs
```

By default this will not generate new certificates unless the existing
ones are about to expire. In cases where the certificates need to be
updated anyway (e.g. because the server configuration changed), this
can be overridden with `--force`.

Generating the certificates requires OpenSSL to be installed.

### Implementation Details

If you wish to manually generate new certificates for any reason, it's
possible to use OpenSSL when starting the server, or starting a test
run, by providing the `--ssl-type=openssl` argument to the `wpt serve`
or `wpt run` commands.

If you installed OpenSSL in such a way that running `openssl` at a
command line doesn't work, you also need to adjust the path to the
OpenSSL binary. This can be done by adding a section to `config.json`
like:

```
"ssl": {"openssl": {"binary": "/path/to/openssl"}}
```

### Windows-specific Instructions

For Windows users, the easiest approach is likely to be using
[WSL](https://docs.microsoft.com/en-us/windows/wsl/) and generate
certificates in a Linux environment. However it is possible to install
OpenSSL and generate the certificates without using WSL.

[Shining Light](https://slproweb.com/products/Win32OpenSSL.html)
provide a convenient installer that is known to work, but requires a
little extra setup:

Run the installer for Win32_OpenSSL_v1.1.0b (30MB). During installation,
change the default location for where to Copy OpenSSL Dlls from the
System directory to the /bin directory.

After installation, ensure that the path to OpenSSL (typically,
this will be `C:\OpenSSL-Win32\bin`) is in your `%Path%`
[Environment Variable](http://www.computerhope.com/issues/ch000549.htm).
If you forget to do this part, you will most likely see a 'File Not Found'
error when you start wptserve.

Finally, set the path value in the server configuration file to the
default OpenSSL configuration file location. To do this, create a file
called `config.json`.  Then add the OpenSSL configuration below,
ensuring that the key `ssl/openssl/base_conf_path` has a value that is
the path to the OpenSSL config file (typically this will be
`C:\\OpenSSL-Win32\\bin\\openssl.cfg`):

```
{
  "ssl": {
    "type": "openssl",
    "encrypt_after_connect": false,
    "openssl": {
      "openssl_binary": "openssl",
      "base_path: "_certs",
      "force_regenerate": false,
      "base_conf_path": "C:\\OpenSSL-Win32\\bin\\openssl.cfg"
    },
  },
}
```
