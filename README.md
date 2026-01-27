
# qupat-pkg

`qupat-pkg` is an utility written in Rust for packaging QGIS plugins and optionally upload 
to package server.

**Important note**: the package is built based on your **last** commit (HEAD) and
discard uncommited changes.

## Note about publishing to QGIS plugin repository

Uploading with `--publish` option is done using a Python script that use the `xml-rpc` 
package from the Python standard libray.  If that is unacceptalbe  for any reason, 
then you must use an alternate method for uploading the plugin archive.

## Variables

* `OSGEO_USERNAME`: the user login for publishing
* `OSGEO_PASSWORD`: the user password for publishing
