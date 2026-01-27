//!
//! Handle parameters
//!
use std::fs::File;
use std::io::Read;
use std::iter;
use std::path::{Path, PathBuf};
use url::Url;

use anyhow::Context;

use crate::errors::Error;

#[derive(Debug, Default)]
pub struct PluginMetadata {
    pub name: String,
    pub author: String,
    pub email: String,
    pub description: String,
    pub tags: Vec<String>,
    pub experimental: bool,
    // NOTE: homepage is mandatory for publishing
    pub homepage: Option<Url>,
    pub tracker: Option<Url>,
    pub repository: Option<Url>,
}

impl PluginMetadata {
    /// Read plugin metadata
    pub fn from_ini(i: &ini::Ini) -> anyhow::Result<Self> {
        let s = i
            .section(Some("general"))
            .ok_or(Error::MissingGeneralSection)?;

        let mut this = Self::default();
        for (k, v) in s.iter() {
            match k {
                "name" => this.name = v.to_string(),
                "author" => this.author = v.to_string(),
                "email" => this.email = v.to_string(),
                "description" => this.description = v.to_string(),
                "tags" => this.tags = v.split(',').map(|s| s.trim().to_string()).collect(),
                "experimental" => this.experimental = v.to_ascii_lowercase().parse()?,
                "homepage" => this.homepage = Some(Url::parse(v)?),
                "tracker" => this.tracker = Some(Url::parse(v)?),
                "repository" => this.repository = Some(Url::parse(v)?),
                _ => (),
            }
        }
        Ok(this)
    }

    pub fn update_config(&self, i: &mut ini::Ini) {
        fn url_to_str(url: &Option<Url>) -> &str {
            url.as_ref().map(|v| v.as_str()).unwrap_or_default()
        }
        i.with_section(Some("general"))
            .set("description", &self.description)
            .set("author", &self.author)
            .set("email", &self.email)
            .set("tags", self.tags.join(","))
            .set("homepage", url_to_str(&self.homepage))
            .set("repository", url_to_str(&self.repository))
            .set("tracker", url_to_str(&self.tracker))
            .set(
                "experimental",
                if self.experimental { "True" } else { "False" },
            );
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct Author {
    name: String,
    email: String,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct ProjectUrls {
    homepage: Option<Url>,
    tracker: Option<Url>,
    repository: Option<Url>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct ProjectMetadata {
    name: String,
    version: String,
    authors: Vec<Author>,
    description: String,
    keywords: Vec<String>,
    urls: ProjectUrls,
    license: String,
    license_files: Vec<String>,
}

impl ProjectMetadata {
    /// Read project's metadata from pyproject.toml file
    pub fn from_project(rootdir: &Path) -> anyhow::Result<Self> {
        let path = rootdir.join("pyproject.toml");
        if path.exists() {
            #[derive(serde::Deserialize)]
            struct PyProject {
                project: ProjectMetadata,
            }

            let mut file = File::open(path)?;
            let mut content = String::new();

            file.read_to_string(&mut content)?;

            Ok(toml::from_str::<PyProject>(&content)?.project)
        } else {
            Ok(Self::default())
        }
    }

    /// Update undefined properties in plugin metadata with
    /// their corresponding values in project metadata
    pub fn update_plugin_metadata(&self, md: &mut PluginMetadata) {
        if !self.authors.is_empty() {
            let author = &self.authors[0];
            if md.author.is_empty() {
                md.author = author.name.clone();
            }
            if md.email.is_empty() {
                md.email = author.email.clone();
            }
        }
        if md.description.is_empty() {
            md.description = self.description.clone();
        }
        if md.tags.is_empty() {
            md.tags = self.keywords.clone();
        }
        if md.homepage.is_none() {
            md.homepage = self.urls.homepage.clone()
        }
        if md.tracker.is_none() {
            md.tracker = self.urls.tracker.clone();
        }
        if md.repository.is_none() {
            md.repository = self.urls.repository.clone();
        }
    }
}

//
// Parameters
//

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    /// The directory name of the source code in the repository
    plugin_source: String,
    /// Changelog file relative to the configuration file.
    /// Defaults to CHANGELOG.md.
    #[serde(default = "Params::default_changelog")]
    changelog_file: String,
    /// Number of changelog entries to add in the metadata.txt
    #[serde(default = "Params::default_changelog_max_entries")]
    changelog_max_entries: usize,
    /// Server RCP (QGIS) endpoint for uploading plugin
    #[serde(default = "Params::default_upload_url")]
    upload_url: Url,
}

impl Params {
    fn default_changelog() -> String {
        "CHANGELOG.md".to_string()
    }
    fn default_changelog_max_entries() -> usize {
        3
    }
    fn default_upload_url() -> Url {
        Url::parse("https://plugins.qgis.org:443/plugins/RPC2/").unwrap()
    }
}

#[derive(Debug)]
pub struct Parameters {
    /// Root directory where is located the configuration file
    rootdir: PathBuf,
    params: Params,
    project_metadata: ProjectMetadata,
    version: semver::Version,
    changelog_file: Option<PathBuf>,
}

impl Parameters {
    // Find candidate config file
    fn find_config_file(rootdir: &Path) -> Result<PathBuf, Error> {
        for file in iter::once("yapt").chain(iter::once("pyproject.toml")) {
            let p = rootdir.join(file);
            if p.exists() {
                return Ok(p);
            }
        }
        Err(Error::NoConfigFile)
    }

    fn read_config_from_file(path: &Path) -> anyhow::Result<Params> {
        let mut file = File::open(path)?;
        let mut content = String::new();

        log::debug!("Reading config from {}", path.display());

        file.read_to_string(&mut content)?;
        let config: toml::Value = toml::from_str(&content)?;

        Ok(
            if path.file_stem().map(|s| s == "pyproject").unwrap_or(false) {
                config.get("tool")
            } else {
                Some(&config)
            }
            .and_then(|v| v.get("yapt"))
            .ok_or(Error::NoConfig)?
            .clone()
            .try_into()?,
        )
    }

    // Constructor
    pub fn load_parameters(rootdir: Option<PathBuf>) -> anyhow::Result<Self> {
        let rootdir = if let Some(rootdir) = rootdir {
            rootdir
        } else {
            std::env::current_dir()?
        };
        let config_path = Self::find_config_file(&rootdir)?;
        let project_metadata = ProjectMetadata::from_project(&rootdir)?;

        // We *require* SemVer compatible versionning scheme
        let version = semver::Version::parse(&project_metadata.version)
            .context("A SemVer compatible version scheme is required")?;

        let params = Self::read_config_from_file(&config_path)?;
        let changelog_file =
            (!params.changelog_file.is_empty()).then(|| rootdir.join(&params.changelog_file));

        if let Some(ref chlg_file) = changelog_file
            && !chlg_file.exists()
        {
            log::warn!("Changelog file '{}' does not exist", chlg_file.display());
        }

        Ok(Self {
            rootdir,
            project_metadata,
            params,
            version,
            changelog_file,
        })
    }

    // Accessors

    pub fn rootdir(&self) -> &Path {
        &self.rootdir
    }

    /*
    pub fn license(&self) -> &str {
        &self.project_metadata.license
    }
    */

    pub fn license_files(&self) -> &[String] {
        self.project_metadata.license_files.as_slice()
    }

    pub fn plugin_source(&self) -> &str {
        &self.params.plugin_source
    }
    pub fn changelog_file(&self) -> Option<&Path> {
        self.changelog_file.as_deref()
    }
    pub fn changelog_max_entries(&self) -> usize {
        self.params.changelog_max_entries
    }
    pub fn upload_url(&self) -> &Url {
        &self.params.upload_url
    }

    //

    #[inline]
    pub fn update_plugin_metadata(&self, md: &mut PluginMetadata) {
        self.project_metadata.update_plugin_metadata(md)
    }

    /// Project version as defined in pyproject.toml
    #[inline]
    pub fn version(&self) -> &semver::Version {
        &self.version
    }

    #[inline]
    pub fn project_name(&self) -> &str {
        &self.project_metadata.name
    }
}

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{fixtures, setup};

    #[test]
    fn test_find_config_file() {
        setup();

        let rootdir = fixtures();
        let path = Parameters::find_config_file(&rootdir).unwrap();
        assert_eq!(path.file_name().unwrap(), "pyproject.toml");

        let path = Parameters::find_config_file(rootdir.parent().unwrap());
        assert!(path.is_err())
    }

    #[test]
    fn test_read_config() {
        setup();

        let rootdir = fixtures();
        let params = Parameters::read_config_from_file(&rootdir.join("pyproject.toml")).unwrap();
        assert_eq!(params.changelog_file, "CHANGELOG.md");
        assert_eq!(params.changelog_max_entries, 3);
    }

    #[test]
    fn test_load_parameters() {
        setup();

        let rootdir = fixtures();
        let p = Parameters::load_parameters(Some(rootdir)).unwrap();

        assert_eq!(
            p.project_metadata.urls.tracker,
            Some(Url::parse("https://github.com/3liz/my-plugin/issues").unwrap()),
        );
        assert_eq!(p.project_metadata.keywords, vec!["tests", "qgis", "plugin"],);
    }
}
