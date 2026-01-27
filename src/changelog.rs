//!
//! Changelog parser
//!
//! Following <https://keepachangelog.com/en/1.0.0/>.
//!

mod parser {

    use nom::{
        IResult, Parser,
        branch::alt,
        bytes::complete::tag,
        character::complete::{alphanumeric0, char, digit1, line_ending, space0, space1},
        combinator::{opt, recognize, value, verify},
        error::ParseError,
        sequence::{delimited, preceded, separated_pair},
    };

    use super::VersionNote;
    use crate::errors::Error;

    fn version_date_sep<'a, E>(sep: char) -> impl Parser<&'a str, Output = &'a str, Error = E>
    where
        E: ParseError<&'a str>,
    {
        recognize((
            verify(digit1, |s: &str| s.len() == 4),
            char(sep),
            verify(digit1, |s: &str| s.len() == 2),
            char(sep),
            verify(digit1, |s: &str| s.len() == 2),
        ))
    }

    fn version_date(i: &str) -> IResult<&str, &str> {
        // Check format YYY-MM-DD
        alt((version_date_sep('-'), version_date_sep('/'))).parse(i)
    }

    /// Parse version as (major.minor.patch, prerelease, buildmetadata))
    pub fn version_tag(i: &str) -> IResult<&str, (&str, &str, &str)> {
        (
            alt((
                delimited(
                    char('['),
                    recognize((digit1, tag("."), digit1, tag("."), digit1)),
                    char(']'),
                ),
                preceded(
                    opt(char('v')),
                    recognize((digit1, tag("."), digit1, tag("."), digit1)),
                ),
            )),
            preceded(opt(tag("-")), alphanumeric0),
            preceded(opt(tag("+")), alphanumeric0),
        )
            .parse(i)
    }

    /// Parse version header as (version_tag, YYY-MM-DD)
    pub fn version_header(i: &str) -> IResult<&str, ((&str, &str, &str), &str)> {
        delimited(
            space0,
            alt((
                value((("Unreleased", "", ""), ""), (tag("Unreleased"), space0)),
                separated_pair(version_tag, (space1, tag("-"), space1), version_date),
            )),
            line_ending,
        )
        .parse(i)
    }

    pub fn parse<'a>(
        i: &'a str,
        unreleased: bool,
    ) -> impl Iterator<Item = Result<VersionNote<'a>, Error>> {
        i.split("\n## ")
            .skip(1)
            .filter(move |i| !i.starts_with("Unreleased") || unreleased)
            .map(|i| match version_header(i) {
                Ok((text_raw, (ver, release_date))) => {
                    let (version, prerelease, buildmetadata) = ver;
                    Ok(VersionNote {
                        version,
                        prerelease,
                        buildmetadata,
                        release_date,
                        text_raw,
                    })
                }
                Err(_) => Err(Error::Changelog(format!("Invalid changelog entry: {}", i))),
            })
    }
}

use crate::errors::Error;
use std::fmt;
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct VersionNote<'a> {
    version: &'a str,
    prerelease: &'a str,
    buildmetadata: &'a str,
    release_date: &'a str,
    text_raw: &'a str,
}

impl<'a> VersionNote<'a> {
    pub fn text(&self) -> &str {
        self.text_raw.trim()
    }

    pub fn match_version(&self, version: &str) -> bool {
        if let Ok((rest, (v, pre, build))) = parser::version_tag(version.trim()) {
            rest == "" && v == self.version && pre == self.prerelease && build == self.buildmetadata
        } else {
            false
        }
    }

    pub fn format_text<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        write!(w, "Version {}", self.version)?;
        if !self.prerelease.is_empty() {
            write!(w, "-{}", self.prerelease)?;
        }
        if !self.buildmetadata.is_empty() {
            write!(w, "+{}", self.buildmetadata)?;
        }
        write!(w, ":\n{}\n\n", self.text())
    }

    pub fn release_date(&self) -> &str {
        self.release_date
    }
}

pub struct Changelog {
    content: String,
}

impl Changelog {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        let mut file = std::fs::File::open(path)?;
        let mut content = String::new();

        file.read_to_string(&mut content)?;
        Ok(Self { content })
    }

    #[inline]
    pub fn versions(&self, count: usize) -> Result<Vec<VersionNote<'_>>, Error> {
        self.iter().take(count).collect()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = Result<VersionNote<'_>, Error>> {
        parser::parse(&self.content, false)
    }

    #[inline]
    pub fn iter_unreleased(&self) -> impl Iterator<Item = Result<VersionNote<'_>, Error>> {
        parser::parse(&self.content, true)
    }

    pub fn format_text<W: fmt::Write>(&self, w: &mut W, count: usize) -> Result<(), Error> {
        self.iter().take(count).try_for_each(|v| {
            v.and_then(|v| {
                v.format_text(w)
                    .map_err(|err| Error::Changelog(format!("{err}")))
            })
        })
    }

    pub fn format_text_unreleased<W: fmt::Write>(
        &self,
        w: &mut W,
        count: usize,
    ) -> Result<(), Error> {
        self.iter_unreleased().take(count).try_for_each(|v| {
            v.and_then(|v| {
                v.format_text(w)
                    .map_err(|err| Error::Changelog(format!("{err}")))
            })
        })
    }

    pub fn note_for(&self, version: &str) -> Option<VersionNote<'_>> {
        self.iter_unreleased().find_map(|v| match v {
            Ok(v) if v.match_version(version) => Some(v),
            Ok(_) => None,
            Err(err) => {
                log::error!("Error parsing changelog {err}");
                None
            }
        })
    }

    #[cfg(test)]
    pub fn from_str(s: &str) -> Self {
        Self {
            content: s.to_string(),
        }
    }
}

//
// Tests
//
//
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{fixtures, setup};

    #[test]
    fn test_changelog_parse_version_header() {
        setup();

        let (rest, (ver, release_date)) =
            parser::version_header(concat!("v1.0.0-alpha1 - 2026-01-01\n",)).unwrap();

        assert_eq!(rest, "");

        let (version, prerelease, buildmetadata) = ver;

        assert_eq!(version, "1.0.0");
        assert_eq!(prerelease, "alpha1");
        assert_eq!(buildmetadata, "");
        assert_eq!(release_date, "2026-01-01");
    }

    #[test]
    fn test_changelog_read_file() {
        setup();

        let changelog = Changelog::read(&fixtures().join("CHANGELOG.md")).unwrap();
        assert!(!changelog.content.is_empty());

        let notes = changelog.versions(3).unwrap();
        assert_eq!(notes.len(), 3);
    }

    #[test]
    fn test_changelog_parse_all() {
        setup();

        let changelog = Changelog::from_str(concat!(
            "# Changelog\n",
            "This is a changelog\n",
            "\n",
            "## v1.0.2 - 2026-03-01\n",
            "\n",
            "- Broke the wrench\n",
            "- Fixed the bath tube\n",
            "\n",
            "## v1.0.1 - 2026-02-01\n",
            "- Walked the dog\n",
            "\n",
            "## v1.0.0 - 2026-01-01\n",
            "- Walked the cat\n",
            "\n",
        ));

        let notes = changelog.versions(3).unwrap();
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].version, "1.0.2");
        assert_eq!(notes[1].version, "1.0.1");
        assert_eq!(notes[2].version, "1.0.0");

        let mut text = String::new();
        changelog.format_text(&mut text, 3).unwrap();
        assert_eq!(
            text,
            concat!(
                "Version 1.0.2:\n- Broke the wrench\n- Fixed the bath tube\n\n",
                "Version 1.0.1:\n- Walked the dog\n\n",
                "Version 1.0.0:\n- Walked the cat\n\n",
            ),
        );
    }
}
