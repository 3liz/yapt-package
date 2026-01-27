#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Missing 'general' section in plugin metadada")]
    MissingGeneralSection,
    #[error("No configuration file found")]
    NoConfigFile,
    #[error("No configuration found")]
    NoConfig,
    #[error("Git command failed")]
    GitFailure,
    #[error("Changelog parse error {0}")]
    Changelog(String),
    #[error("Package error")]
    InvalidPackageFile,
    #[error("Upload failed")]
    UploadFailure,
    #[error("Missing username")]
    MissingUserName,
    #[error("Missing password")]
    MissingPassword,
}
