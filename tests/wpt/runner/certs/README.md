## To regenerate certificates

1. Apply:

```
diff --git a/tools/wptserve/wptserve/sslutils/openssl.py b/tools/wptserve/wptserve/sslutils/openssl.py
index 87a8cc9cc7..bbf500d8ca 100644
--- a/tools/wptserve/wptserve/sslutils/openssl.py
+++ b/tools/wptserve/wptserve/sslutils/openssl.py
@@ -216,7 +216,7 @@ basicConstraints = CA:true
 subjectKeyIdentifier=hash
 authorityKeyIdentifier=keyid:always,issuer:always
 keyUsage = keyCertSign
-%(constraints_line)s
+#%(constraints_line)s
 """ % {"root_dir": root_dir,
        "san_line": san_line,
        "duration": duration,
```

2. `cd tests/wpt/suite/`
3. `./wpt serve --config tools/certs/config.json`
4. Run:

```
cp tests/wpt/suite/tools/certs/cacert.key tests/wpt/runner/certs/cacert.key
cp tests/wpt/suite/tools/certs/cacert.pem tests/wpt/runner/certs/cacert.pem
cp tests/wpt/suite/tools/certs/web-platform.test.key tests/wpt/runner/certs/web-platform.test.key
cp tests/wpt/suite/tools/certs/web-platform.test.pem tests/wpt/runner/certs/web-platform.test.pem
```
