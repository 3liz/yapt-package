//
// Git helper
//
use crate::errors::Error;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub struct Git {
    command: Command,
}

impl Git {
    fn new(rootdir: &Path) -> Self {
        let mut command = Command::new("git");
        command.current_dir(rootdir);

        Self { command }
    }

    fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.command.arg(arg);
        self
    }

    fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command.args(args);
        self
    }

    fn run(&mut self) -> anyhow::Result<Vec<u8>> {
        let output = self.command.output()?;

        std::io::stderr().write_all(&output.stderr)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            std::io::stdout().write_all(&output.stdout)?;
            Err(Error::GitFailure.into())
        }
    }

    /// Archive file for HEAD
    /// Note: source is *relative to the rootdir
    pub fn archive(rootdir: &Path, output: &Path, source: &Path) -> anyhow::Result<()> {
        Git::new(rootdir)
            .args(["archive", "HEAD", "-o"])
            .arg(output.as_os_str())
            .args(["--format", "zip"])
            .arg(source.as_os_str())
            .run()?;

        Ok(())
    }

    /// Return the HEAD commit
    pub fn commit_sha1(rootdir: &Path) -> anyhow::Result<String> {
        Ok(
            String::from_utf8_lossy(&Git::new(rootdir).args(["rev-parse", "HEAD"]).run()?)
                .trim_end()
                .to_string(),
        )
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
    fn test_git_commit() {
        setup();
        let rootdir = fixtures();
        let commit = Git::commit_sha1(&rootdir).unwrap();
        assert_eq!(commit.len(), 40);
    }

    #[test]
    fn test_git_archive() {
        setup();
        let rootdir = fixtures();
        let output = rootdir.parent().unwrap().join("git_test.zip");

        if output.exists() {
            std::fs::remove_file(&output).unwrap();
        }
        assert!(!output.exists());

        Git::archive(&rootdir, &output, Path::new("my_plugin")).unwrap();
        assert!(output.exists());
    }
}
