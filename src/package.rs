//!
//! Packaging
//!
use crate::changelog::Changelog;
use crate::git::Git;
use crate::parameters::{Parameters, PluginMetadata};

use anyhow::Context;
use std::path::{Path, PathBuf};

use crate::errors::Error;

#[derive(Default)]
pub struct Options {
    /// Force prerelease,
    pub prerelease: bool,
    /// Archive destination,
    pub output_dir: Option<PathBuf>,
    /// Keep source of intermediate build
    pub keep_intermediate_files: bool,
}

// =====================
// Create plugin archive
// =====================
pub fn create_archive(
    parameters: &Parameters,
    opts: Options,
) -> anyhow::Result<(PathBuf, Option<PathBuf>)> {
    // Uncompress archive to temporary directory
    let project_slug = slug::slugify(parameters.project_name());
    let tmpdir = tempfile::TempDir::with_prefix_in(
        format!("pkg-build-{project_slug}-"),
        parameters.rootdir(),
    )
    .context("Failed to create temporary directory")?;

    let git_archive = create_git_archive(parameters, tmpdir.path())?;

    // Uncompress archive in tmpdir
    zip::ZipArchive::new(std::fs::File::open(git_archive)?)
        .context("Failed to open git archive")?
        .extract(tmpdir.path())
        .context("Failed to extract git archive")?;

    let source = tmpdir.path().join(parameters.plugin_source());
    let release_version = if opts.prerelease {
        get_prerelease_version(parameters.version())
    } else {
        parameters.version().to_string()
    };

    let metadata = update_plugin_metadata(parameters, &source, &release_version, &opts)?;

    // Copy extra files
    copy_license_files(parameters, &source)?;
    copy_i8n_file(parameters, &source)?;

    // Create the final archive
    let archive = make_archive(&metadata.name, &release_version, &source, &opts)?;

    if opts.keep_intermediate_files {
        Ok((archive, Some(tmpdir.keep())))
    } else {
        Ok((archive, None))
    }
}

// Read changelog
pub fn read_changelog(parameters: &Parameters) -> anyhow::Result<Option<Changelog>> {
    if let Some(path) = parameters.changelog_file()
        && path.exists()
    {
        log::debug!("Reading changelog: {}", path.display());
        Ok(Some(
            Changelog::read(path).context("Failed to read changelog")?,
        ))
    } else {
        Ok(None)
    }
}

// Create final zip archive

fn make_archive(
    name: &str,
    version: &str,
    source: &Path,
    opts: &Options,
) -> anyhow::Result<PathBuf> {
    // Create archive
    let archive_name = format!("{}.{version}.zip", slug::slugify(name));
    log::info!("Creating package");
    let output = if let Some(output_dir) = &opts.output_dir {
        output_dir.join(archive_name)
    } else {
        Path::new(&archive_name).to_path_buf()
    };

    log::debug!("Creating archive {}", output.display());

    let archive = zip::ZipWriter::new(std::fs::File::create(&output)?);

    fn make_archive_safe(
        source: &Path,
        mut zip: zip::ZipWriter<std::fs::File>,
    ) -> anyhow::Result<()> {
        let options = zip::write::SimpleFileOptions::default();
        // SECURITY NOTE: Any error after this point may leave a partial or corrupt
        // Create base dir

        let plugin_source_name = source.file_name().unwrap().to_str().unwrap();
        zip.add_directory(plugin_source_name, options)?;

        let prefix = source.parent().unwrap_or_else(|| Path::new("."));

        for entry in glob::glob(&format!("{}/**/*", source.display()))? {
            let path = entry?;
            log::trace!("Adding file {}", path.display());

            let name = path.strip_prefix(prefix)?;
            let path_as_string = name.to_str().ok_or_else(|| {
                log::error!("Invalid path {name:?}");
                Error::InvalidPackageFile
            })?;

            if path.is_file() {
                zip.start_file(path_as_string, options)?;
                let mut f = std::fs::File::open(path)?;

                std::io::copy(&mut f, &mut zip)?;
            } else if path.is_dir() {
                zip.add_directory(path_as_string, options)?;
            }
        }
        zip.finish()?;
        Ok(())
    }

    make_archive_safe(source, archive).inspect_err(|_| {
        if let Err(err) = std::fs::remove_file(&output) {
            log::error!("Failed to remove {}: {err}", output.display());
        }
    })?;

    Ok(output)
}

// Update metadata
fn update_plugin_metadata(
    parameters: &Parameters,
    source: &Path,
    version_str: &str,
    opts: &Options,
) -> anyhow::Result<PluginMetadata> {
    let metadata_file = source.join("metadata.txt");

    let mut config = ini::Ini::load_from_file_opt(
        &metadata_file,
        ini::ParseOption {
            enabled_indented_multiline_value: true,
            ..Default::default()
        },
    )
    .context("Failed to parse metadata file")?;

    let mut metadata = PluginMetadata::from_ini(&config)?;

    // Update metadata with project metadata
    parameters.update_plugin_metadata(&mut metadata);

    let plugin_version = parameters.version();

    // Update plugin metadata
    metadata.experimental = opts.prerelease || !plugin_version.pre.is_empty();
    metadata.update_config(&mut config);

    let date_time = time::OffsetDateTime::now_utc()
        .format(&time::format_description::parse_strftime_borrowed("%Y-%m-%dT%H:%M:%SZ").unwrap())
        .unwrap();

    config
        .with_section(Some("general"))
        .set("version", version_str)
        .set("commitSha1", Git::commit_sha1(parameters.rootdir())?)
        .set("dateTime", date_time)
        .set("changelog", changelog_text(parameters, opts.prerelease)?);

    // Write back metadata file
    config
        .write_to_file_opt(
            metadata_file,
            ini::WriteOption {
                indent_multiline_value: true,
                ..Default::default()
            },
        )
        .context("Failed to write plugin metadata file")?;

    Ok(metadata)
}

// Build the changelog textual information
// If unreleased == true then add the unreleased section
// into the changelog text
fn changelog_text(parameters: &Parameters, unreleased: bool) -> anyhow::Result<String> {
    if let Some(changelog) = read_changelog(parameters)? {
        let mut text = String::from("\n");
        if unreleased {
            changelog.format_text_unreleased(&mut text, parameters.changelog_max_entries())?;
        } else {
            changelog.format_text(&mut text, parameters.changelog_max_entries())?;
        }
        Ok(text)
    } else {
        Ok(String::new())
    }
}

// Force a prerelease version
fn get_prerelease_version(version: &semver::Version) -> String {
    if version.pre.is_empty() {
        let mut version = version.clone();
        version.pre = semver::Prerelease::new("alpha").unwrap();
        version.to_string()
    } else {
        version.to_string()
    }
}

// Create an intermediate git archive with all plugin files
fn create_git_archive(parameters: &Parameters, dir: &Path) -> anyhow::Result<PathBuf> {
    let git_archive = dir.join("git.zip");
    // Create archive for HEAD
    log::debug!("Creating git archive {}", git_archive.display());
    Git::archive(
        parameters.rootdir(),
        &git_archive,
        Path::new(parameters.plugin_source()),
    )?;

    Ok(git_archive)
}

fn copy_license_files(parameters: &Parameters, source: &Path) -> std::io::Result<()> {
    let rootdir = parameters.rootdir();
    parameters.license_files().iter().try_for_each(|s| {
        let path = rootdir.join(s);
        if path.exists() {
            let dest = source.join(path.file_name().unwrap());
            log::debug!("Copying license file {}", path.display());
            std::fs::copy(&path, &dest)?;
        }
        Ok(())
    })
}

// Copy .qm files as they are usually not under version control
fn copy_i8n_file(parameters: &Parameters, source: &Path) -> anyhow::Result<()> {
    let src = parameters
        .rootdir()
        .join(parameters.plugin_source())
        .join("i18n");

    if src.exists() {
        let dst = source.join("i18n");
        if !dst.exists() {
            std::fs::create_dir(&dst)?;
        }
        for file in glob::glob(&format!("{}/*.qm", src.display()))? {
            let file = file?;
            std::fs::copy(&file, dst.join(file.file_name().unwrap()))?;
        }
    }
    Ok(())
}

//
// Tests
//
//
#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::create_archive;
    use crate::parameters::Parameters;
    use crate::tests::{fixtures, setup};

    #[test]
    fn test_package_changelog_text() {
        setup();

        let rootdir = fixtures();
        let parameters = Parameters::load_parameters(Some(rootdir)).unwrap();

        assert_eq!(parameters.changelog_max_entries(), 3);

        let text = changelog_text(&parameters, false).unwrap();
        assert_eq!(
            text,
            concat!(
                "\n",
                "Version 10.1.0-beta1:\n",
                "- This is the latest documented version in this changelog\n",
                "- The changelog module is tested against these lines\n",
                "- Be careful modifying this file\n\n",
                "Version 10.1.0-alpha1:\n",
                "- This is a version with a prerelease in this changelog\n",
                "- The changelog module is tested against these lines\n",
                "- Be careful modifying this file\n\n",
                "### Fixed\n\n",
                "- trying with a subsection in a version note\n\n",
                "Version 10.0.1:\n",
                "- End of year version\n\n",
            ),
        );
    }

    #[test]
    fn test_package_create_archive() {
        setup();

        let rootdir = fixtures();
        let parameters = Parameters::load_parameters(Some(rootdir)).unwrap();

        let (output, tmpdir) = create_archive(
            &parameters,
            Options {
                prerelease: true,
                keep_intermediate_files: true,
                ..Default::default()
            },
        )
        .unwrap();

        let tmpdir = tmpdir.unwrap();
        let source = tmpdir.join(parameters.plugin_source());
        assert!(source.exists());
        assert!(source.join("LICENSE").exists());

        // Check metadata file
        let md = ini::Ini::load_from_file_opt(
            source.join("metadata.txt"),
            ini::ParseOption {
                enabled_indented_multiline_value: true,
                ..Default::default()
            },
        )
        .unwrap();

        let s = md.section(Some("general")).unwrap();
        assert_eq!(s.get("version"), Some("10.1.0-beta1"));

        // Check output archive
        assert!(output.exists());
    }

    #[test]
    fn test_load_multiline_ini() {
        let multi = concat!(
            "[foo]\n",
            "bar=\n",
            "\tTake it to the sea\n",
            "\tHello world\n",
            "\t\n",
            "\tbye\n",
        );

        let md = ini::Ini::load_from_str_opt(
            multi,
            ini::ParseOption {
                enabled_indented_multiline_value: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            md.get_from(Some("foo"), "bar"),
            Some("Take it to the sea\nHello world\n\nbye"),
        );
    }

    #[test]
    fn test_save_multiline_ini() {
        let multi = "\nTake it to the sea\nHello world\n\nbye";

        let mut md = ini::Ini::new();
        md.set_to(Some("foo"), "bar".to_string(), multi.to_string());

        assert_eq!(
            md.get_from(Some("foo"), "bar"),
            Some("\nTake it to the sea\nHello world\n\nbye"),
        );
    }
}
