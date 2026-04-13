# yapt-pkg

## Overview

`yapt-pkg` is a Rust-based CLI tool for packaging QGIS plugins. It creates a zip archive from a plugin source tree, optionally includes changelog entries in `metadata.txt`, and can publish directly to the QGIS plugin repository.

**Important note**: `yapt-pkg` relies on the `git archive` command to create archive, that means that package is built based on your **last** commit (HEAD) and discard uncommited changes.


## Configuration

Because QGIS plugins are regular Python packages, `yapt-pkg` enforce the usage of `pyproject.toml`
configuration file.

`yapt-pkg` looks for a configuration file in this order:
1. `yapt` (TOML file)
2. `pyproject.toml` (expects `[tool.yapt]` section)

Required configuration under `[tool.yapt]`:
- `plugin_source`: directory name containing the plugin source code

Optional settings:
- `changelog_file`: defaults to `CHANGELOG.md`
- `changelog_max_entries`: number of changelog entries to include (default: 3)
- `upload_url`: QGIS plugin server endpoint (default: `https://plugins.qgis.org:443/plugins/RPC2/`)

Example `pyproject.toml`:
```toml
[project]
name = "my-plugin"
version = "1.0.0"

[tool.yapt]
plugin_source = "my_plugin"
```

### Configuring metadata 

By default `yapt-pkg` use the `metadata.txt` provided in the plugin source directory. 

If some properties are no set in the `metadata.txt` they are imported from the `pyproject.toml`
file:

| `pyproject.toml` | `metadata.txt` |
| ---------------- | -------------- |
| projoct.version          | general/version       |
| project.authors.name     | general/author *(1)*  |
| project.authors.email    | general/email *(1)*   |
| project.url.description  | general/description   |
| project.url.keywords     | general/tags          |
| project.url.homepage     | general/homepage      |
| project.url.repository   | general/repository    |
| project.url.tracker      | general/tracker       |

*Notes:*
* *(1)*: Only from the first `[[project.authors]]` entry.


### Versioning scheme

`qgis-pkg` *requires* a SemVer compatible version scheme. 

This scheme is widely used and a has a lot of parsing tools in many languages - which is not the case
with `PEP 440` which is a loose format wich has more complex precedence rules.

See the [Python versioning discussion](https://packaging.python.org/en/latest/specifications/version-specifiers/#semantic-versioning) for comparison between `PEP 440` rules and Semantic Versioning.

---

## CLI Commands

### 1. `package` — Create plugin archive

Creates a zip archive suitable for QGIS plugin upload.

```bash
yapt-pkg package
```

**Options:**

| Option | Description |
|--------|-------------|
| `--pre` | Force prerelease version (e.g., `1.0.0-beta1`) |
| `-o, --output <DIR>` | Output directory for the archive |
| `--keep` | Keep intermediate build files |
| `--publish` | Publish to QGIS plugin repository |
| `--xml <URL>` | Generate QGIS package XML with download URL |
| `--dry-run` | Test connection without publishing |
| `--osgeo-username` | QGIS repo username (or set `OSGEO_USERNAME` env) |
| `--osgeo-password` | QGIS repo password (or set `OSGEO_PASSWORD` env) |

**Examples:**

```bash
# Basic package creation
yapt-pkg package

# Create archive with prerelease version
yapt-pkg package --pre

# Create archive in specific directory
yapt-pkg package -o ./dist

# Generate package XML for QGIS
yapt-pkg package --xml "https://example.com/downloads/"

# Publish to QGIS plugin repository
yapt-pkg package --publish --osgeo-username "user" --osgeo-password "pass"

# Publish with environment variables
OSGEO_USERNAME=user OSGEO_PASSWORD=pass yapt-pkg package --publish

# Dry run (test connection)
yapt-pkg package --publish --dry-run
```

---

### 2. `changelog` — Get changelog entry

Returns the changelog text for a specific version.

```bash
yapt-pkg changelog
```

**Options:**

| Option | Description |
|--------|-------------|
| `--version <VER>` | Specific version to get changelog for |

**Examples:**

```bash
# Get changelog for current version (from pyproject.toml)
yapt-pkg changelog

# Get changelog for specific version
yapt-pkg changelog --version "1.0.0"
```

---

### Global options

| Option | Description |
|--------|-------------|
| `--rootdir <DIR>` | Plugin root directory (default: current directory) |
| `-v, --verbose` | Increase verbosity (can stack: `-v`, `-vv`, `-vvv`) |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

---

## Note about publishing to QGIS official plugin repository

Uploading with `--publish` option is done using a Python script that use the `xml-rpc` 
package from the Python standard libray.  If that is unacceptalbe  for any reason, 
then you must use an alternate method for uploading the plugin archive.

## Variables

* `OSGEO_USERNAME`: the user login for publishing on qgis.org
* `OSGEO_PASSWORD`: the user password for publishing qgis.org
