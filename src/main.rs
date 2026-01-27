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
mod package;
mod parameters;
mod publish;

#[derive(Parser)]
#[command(version, author, about, long_about=None)]
#[command(arg_required_else_help = true)]
#[command(styles = CLAP_STYLE)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
    #[arg(long)]
    rootdir: Option<PathBuf>,
}

/// Commands
#[derive(Subcommand)]
enum Commands {
    /// Returns the changelog content
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
        let version = self.version.unwrap_or_else(|| parameters.version().to_string());
        if let Some(changelog) = package::read_changelog(&parameters)? {
            if let Some(note) = changelog.note_for(&version) {
                eprintln!("Found changelog for version {version}");
                write!(std::io::stdout(), "{}\n", note.text())?;
            } else {
                eprintln!("No changelog found for version {version}");
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
    #[arg(long)]
    osgeo_username: Option<String>,
    /// The Osgeo password (only with --publish)
    #[arg(long)]
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
            eprintln!(
                concat!(
                    "Intermediate files kept at {}\n",
                    "Please remember to delete them.",
                ),
                kept_files.display(),
            );
        }

        println!("Archive created at {}", archive.display());

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
        let username = self
            .osgeo_username
            .take()
            .or_else(|| std::env::var("OSGEO_USERNAME").ok())
            .ok_or(Error::MissingUserName)?;

        let password = self
            .osgeo_password
            .take()
            .or_else(|| std::env::var("OSGEO_PASSWORD").ok())
            .ok_or(Error::MissingPassword)?;

        eprintln!("Publishing package...");
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

    let mut builder = env_logger::Builder::from_env(Env::default().default_filter_or("warn"));

    match verbosity {
        1 => builder.filter_level(LevelFilter::Info),
        2 => builder.filter_level(LevelFilter::Debug),
        _ if verbosity > 2 => builder.filter_level(LevelFilter::Trace),
        _ => &mut builder,
    }
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
