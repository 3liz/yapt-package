//!
//! Publish plugin to remote server
//!
//! Note, there is no really up to date xml-rpc client in Rust. And the
//! xmlrpc module is in the python standard library.
//!
//! So, don't reinvent the wheel and rely simply in Python call to upload
//! the plugin to the Qgis server since we don't have to rely on dependencies.
//!
//! Furthemore, it is not obvious that the QGIS plugin server will, one day,
//! evolves to a decent REST api. From this day, will reconsider implement
//! the upload in pure rust.
//!
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::changelog::Changelog;
use crate::errors::Error;
use crate::parameters::Parameters;

#[derive(Default)]
pub struct Options {
    pub username: String,
    pub password: String,
    pub dry_run: bool,
}

const PYTHON_SCRIPT: &str = include_str!("upload.py");

pub fn publish(parameters: &Parameters, archive: &Path, opts: Options) -> anyhow::Result<()> {
    let auth_string = format!("{}:{}", opts.username, opts.password);

    let upload_url = parameters.upload_url().as_str();
    log::info!(
        "Uploading {} to plugin repository: {upload_url}",
        archive.display()
    );

    let verbose = if log::log_enabled!(log::Level::Debug) {
        "debug"
    } else {
        ""
    };
    let python_exec = std::env::var_os("PYTHON_EXECUTABLE")
        .unwrap_or_else(|| OsStr::new("python3").to_os_string());
    let output = Command::new(python_exec)
        .args(["-c", PYTHON_SCRIPT])
        .args([upload_url, &auth_string])
        .arg(archive)
        .env("XML_RPC_DRY_RUN", if opts.dry_run { "yes" } else { "" })
        .env("XML_RPC_LOG", verbose)
        .output()?;

    std::io::stderr().write_all(&output.stderr)?;
    if !output.status.success() {
        std::io::stdout().write_all(&output.stdout)?;
        return Err(Error::UploadFailure.into());
    } else if !verbose.is_empty() {
        std::io::stdout().write_all(&output.stdout)?;
    }
    Ok(())
}

/// Generate QGIS plugin repository XML
pub fn generate_xml(
    archive: &Path,
    download_url: &str,
    username: &str,
    changelog: Option<Changelog>,
) -> anyhow::Result<()> {
    let metadata = read_metadata_from_archive(archive)?;
    let s = metadata
        .section(Some("general"))
        .ok_or(Error::MissingGeneralSection)?;

    let update_date = time::OffsetDateTime::now_utc()
        .format(&time::format_description::parse_strftime_borrowed("%Y-%m-%d").unwrap())
        .unwrap();

    let file_name = archive
        .file_name()
        .unwrap()
        .to_str()
        .expect("Archive file name not UTF-8");

    // Get the first release date
    let create_date = changelog
        .and_then(|changelog| {
            changelog
                .versions(999)
                .inspect_err(|err| log::error!("{err}"))
                .ok()
                .and_then(|versions| versions.last().map(|note| note.release_date().to_string()))
        })
        .unwrap_or_default();

    let get = |k| s.get(k).unwrap_or_default();

    let mut file = std::fs::File::create(archive.with_extension("xml"))?;
    write!(
        file,
        concat!(
            "<pyqgis_plugin name=\"{name}\" version=\"{version}\">\n",
            "    <description><![CDATA[{description}]]></description>\n",
            "    <version>{version}</version>\n",
            "    <qgis_minimum_version>{qgis_minimum_version}</qgis_minimum_version>\n",
            "    <qgis_maximum_version>{qgis_maximum_version}</qgis_maximum_version>\n",
            "    <homepage>{homepage}</homepage>\n",
            "    <file_name>{file_name}</file_name>\n",
            "    <icon>{icon}</icon>\n",
            "    <author_name>{author_name}</author_name>\n",
            "    <download_url>{download_url}</download_url>\n",
            "    <uploaded_by>{username}</uploaded_by>\n",
            "    <create_date>{create_date}</create_date>\n",
            "    <update_date>{update_date}</update_date>\n",
            "    <experimental>{experimental}</experimental>\n",
            "    <deprecated>{deprecated}</deprecated>\n",
            "    <tracker>{tracker}</tracker>\n",
            "    <repository>{repository}</repository>\n",
            "    <tags>{tags}</tags>\n",
            "    <server>{server}</server>\n",
            "</pyqgis_plugin>\n",
        ),
        file_name = file_name,
        download_url = download_url,
        username = username,
        update_date = update_date,
        create_date = create_date,
        name = get("name"),
        version = get("version"),
        description = get("description"),
        qgis_minimum_version = get("qgisMinimumVersion"),
        qgis_maximum_version = s.get("qgisMaximumVersion").unwrap_or("3.99"),
        homepage = get("homepage"),
        icon = get("icon"),
        author_name = get("author"),
        experimental = s.get("experimental").unwrap_or("False"),
        deprecated = s.get("deprecated").unwrap_or("False"),
        tracker = get("tracker"),
        repository = get("repository"),
        tags = get("tags"),
        server = s.get("server").unwrap_or("False"),
    )?;

    Ok(())
}

fn read_metadata_from_archive(archive: &Path) -> anyhow::Result<ini::Ini> {
    // Read the metadata from the archive
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;

    let root = zip
        .root_dir(zip::read::root_dir_common_filter)?
        .ok_or(Error::InvalidPackageFile)?;

    let mut metadata = zip
        .by_path(root.as_path().join("metadata.txt"))
        .map_err(|err| {
            log::error!("{err}");
            Error::InvalidPackageFile
        })?;

    Ok(ini::Ini::read_from(&mut metadata)?)
}
