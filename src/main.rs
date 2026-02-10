//!
//! Qgis plugin package ci
//!

use clap::{ArgAction, Args, Parser, Subcommand};

use std::io::Write;
use std::path::{Path, PathBuf};

use errors::Error;
use parameters::Parameters;

mod changelog;
mod errors;
mod git;
mod notice;
mod package;
mod parameters;
mod publish;

#[derive(Parser)]
#[command(version, author, about, long_about=None)]
#[command(
    disable_help_flag = true,
    disable_help_subcommand = false,
    disable_version_flag = true
)]
#[command(styles = CLAP_STYLE)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// The root directory for the plugin sources
    ///
    /// Usually where the `pyproject.toml` file is located.
    #[arg(global = true, long, help_heading = "Project options")]
    rootdir: Option<PathBuf>,
    #[arg(
        global = true,
        short,
        long,
        action = ArgAction::Count,
        help_heading = "Global options",
    )]
    verbose: u8,
    #[arg(global = true, short, long, action = ArgAction::HelpShort, help_heading = "Global options")]
    help: Option<bool>,
    #[arg(short = 'V', long, action = ArgAction::Version, help_heading = "Global options")]
    version: Option<bool>,
}

/// Commands
#[derive(Subcommand)]
enum Commands {
    /// Returns the changelog entry for
    /// a given version
    Changelog(CmdChangelog),
    /// Create plugin archive
    Package(CmdPackage),
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    use Commands::*;

    init_logger(args.verbose);

    match args.command {
        Changelog(cmd) => cmd.execute(args.rootdir),
        Package(cmd) => cmd.execute(args.rootdir),
    }
}

//
// Changelog
//

#[derive(Args)]
#[command()]
struct CmdChangelog {
    /// Get changelog for specific version
    #[arg(long)]
    version: Option<String>,
}

impl CmdChangelog {
    fn execute(self, rootdir: Option<PathBuf>) -> anyhow::Result<()> {
        let parameters = Parameters::load_parameters(rootdir)?;
        let version = self
            .version
            .unwrap_or_else(|| parameters.version().to_string());
        if let Some(changelog) = package::read_changelog(&parameters)? {
            if let Some(note) = changelog.note_for(&version) {
                notice::info!("{} {}", "Found changelog for version", version,);
                writeln!(std::io::stdout(), "\n{}\n", note.text())?;
            } else {
                notice::info!("{} {}", "No changelog found for version", version,);
            }
        }
        Ok(())
    }
}

//
// Package
//

#[derive(Args)]
#[command()]
struct CmdPackage {
    /// Force prerelease
    #[arg(long)]
    pre: bool,
    /// Output directory
    #[arg(long, short)]
    output: Option<PathBuf>,
    /// Keep intermediate files
    #[arg(long)]
    keep: bool,
    /// Publish the package to plugin repository
    #[arg(long)]
    publish: bool,
    /// The Osgeo user name (only with --publish)
    #[arg(long, env = "OSGEO_USERNAME")]
    osgeo_username: Option<String>,
    /// The Osgeo password (only with --publish)
    #[arg(long, env = "OSGEO_PASSWORD")]
    osgeo_password: Option<String>,
    /// Check only server connection without publishing
    #[arg(long)]
    dry_run: bool,
    /// Generate a QGIS package XML
    #[arg(long, name = "DOWNLOAD_URL")]
    xml: Option<String>,
}

impl CmdPackage {
    fn execute(mut self, rootdir: Option<PathBuf>) -> anyhow::Result<()> {
        let parameters = Parameters::load_parameters(rootdir)?;
        let (archive, kept_files) = package::create_archive(
            &parameters,
            package::Options {
                prerelease: self.pre,
                output_dir: self.output.take(),
                keep_intermediate_files: self.keep,
            },
        )?;

        if let Some(kept_files) = kept_files {
            notice::info!(
                "{} {}\n{}",
                "Intermediate files kept at",
                kept_files.display(),
                "Please remember to delete them.",
            );
        }

        notice::info!("{} {}", "Archive created at", archive.display());

        if let Some(ref download_url) = self.xml {
            let username = self.osgeo_username.as_ref().map_or("", |v| v.as_str());
            publish::generate_xml(
                &archive,
                download_url,
                username,
                package::read_changelog(&parameters)?,
            )?;
        }

        if self.publish {
            self.do_publish(&parameters, &archive)?;
        }
        Ok(())
    }

    fn do_publish(&mut self, parameters: &Parameters, archive: &Path) -> anyhow::Result<()> {
        let username = self.osgeo_username.take().ok_or(Error::MissingUserName)?;

        let password = self.osgeo_password.take().ok_or(Error::MissingPassword)?;

        notice::info!("{}", "Publishing package...");
        publish::publish(
            parameters,
            archive,
            publish::Options {
                username,
                password,
                dry_run: self.dry_run,
            },
        )?;

        Ok(())
    }
}

//
// Logger
//

fn init_logger(verbosity: u8) {
    use env_logger::Env;
    use log::LevelFilter;

    env_logger::Builder::from_env(Env::default())
        .format_timestamp(None)
        .format_target(verbosity > 2)
        .filter_level(match verbosity {
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ if verbosity > 2 => LevelFilter::Trace,
            _ => LevelFilter::Warn,
        })
        .init();
}

// Clap style

use clap::builder::styling;

const CLAP_STYLE: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::Green.on_default().bold())
    .usage(styling::AnsiColor::Green.on_default().bold())
    .literal(styling::AnsiColor::Blue.on_default().bold())
    .placeholder(styling::AnsiColor::Cyan.on_default());

#[cfg(test)]
mod tests;
