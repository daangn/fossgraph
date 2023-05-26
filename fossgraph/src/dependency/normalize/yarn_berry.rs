use std::collections::HashMap;

use crate::dependency::Dependency;

use fancy_regex::Regex;
use lazy_static::lazy_static;
use serde_yaml::Value;
use url::Url;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Couldn't parse the lockfile.\n{message}")]
    InvalidLockfileFormat { message: String },
}

impl From<serde_yaml::Error> for Error {
    fn from(_error: serde_yaml::Error) -> Self {
        Self::invalid_yaml()
    }
}

impl Error {
    fn invalid_yaml() -> Self {
        Self::InvalidLockfileFormat {
            message: "Not a valid YAML".into(),
        }
    }

    fn invalid_format() -> Self {
        Self::InvalidLockfileFormat {
            message: "Malformed lockfile".into(),
        }
    }

    fn invalid_descriptor(descriptor: &str) -> Self {
        Self::InvalidLockfileFormat {
            message: format!("Some resolution has unsupported descriptor: {}", descriptor),
        }
    }
}

// Resolution string parsing rule:
//
// resolution is a valid descriptor
//
// "descriptor" follows form of `<ident>(@<range>)`
// "ident" is package name (e.g. lodash, @types/lodash)
// "range" follows form of `<protocol>:<selector>(#<source>)(::<bindings>)`
//    One eception here is `git` and `github` protocl have other forms.
//
// "selector" is a fixed version on a resolution

#[derive(Debug, PartialEq, Eq)]
enum PackageDescriptor {
    Regular {
        ident: String,
        range: PackageRange,
    },
    Git {
        ident: String,
        url: String, // there is no support for `ssh://` or `file://`.
        commit_hash: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
struct PackageRange {
    /// Note: protocol always ends with ':'. (e.g. "npm:")
    protocol: String,
    selector: String,
    source: Option<String>,
    bindings: Option<HashMap<String, String>>,
}

impl TryFrom<&str> for PackageDescriptor {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (ident, range) = Self::split_range(value)?;
        if range.starts_with("git@") || range.starts_with("https://github.com") {
            if let Some((url, commit_hash)) = range.split_once("#commit=") {
                Ok(Self::Git {
                    ident: ident.into(),
                    url: url.into(),
                    commit_hash: commit_hash.into(),
                })
            } else {
                Err(Error::invalid_descriptor(value))
            }
        } else {
            Ok(Self::Regular {
                ident: ident.into(),
                range: Self::parse_range(value, range)?,
            })
        }
    }
}

impl PackageDescriptor {
    fn split_range<'a>(descriptor: &'a str) -> Result<(&'a str, &'a str), Error> {
        if descriptor.starts_with("@") {
            if let Some((index, _)) = descriptor.match_indices('@').skip(1).next() {
                let (ident, _) = descriptor.split_at(index);
                let (_, range) = descriptor.split_at(index + 1);
                Ok((ident, range))
            } else {
                Err(Error::invalid_descriptor(descriptor))
            }
        } else {
            if let Some((ident, range)) = descriptor.split_once("@") {
                Ok((ident, range))
            } else {
                Err(Error::invalid_descriptor(descriptor))
            }
        }
    }

    fn parse_range(descriptor: &str, range: &str) -> Result<PackageRange, Error> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                "^(?<protocol>[^#:\\s]*:)(?<selector>(?:(?!::)[^#\\s])*)(?:#(?<source>(?:(?!::).)*))?(?:::(?<bindings>.*))?$",
            ).unwrap();
        }
        if let Some(captures) = RE.captures(range).unwrap() {
            let protocol = captures.name("protocol").unwrap().as_str().to_owned();
            let selector = captures.name("selector").unwrap().as_str().to_owned();
            let source = captures.name("source").map(|m| m.as_str().to_owned());
            let bindings = captures.name("bindings").map(|m| {
                let dummy_url = "http://dummy?".to_owned() + m.as_str();
                let parsed = Url::parse(dummy_url.as_str()).unwrap();
                return parsed
                    .query_pairs()
                    .into_owned()
                    .collect::<HashMap<String, String>>();
            });
            Ok(PackageRange {
                protocol,
                selector,
                source,
                bindings,
            })
        } else {
            Err(Error::invalid_descriptor(descriptor))
        }
    }
}

fn collect_from_yaml(content: Value) -> Result<Vec<Dependency>, Error> {
    match content.as_mapping() {
        Some(map) => {
            let mut iter = map.iter();
            let (_key, _value) = iter.next().unwrap(); // skip metadata
            for (_key, value) in iter {
                // TODO: collect resolutions
            }
            Ok(vec![])
        }
        None => Err(Error::InvalidLockfileFormat {
            message: "A mapping is expected".into(),
        }),
    }
}

fn collect_from_str(content: &str) -> Result<Vec<Dependency>, Error> {
    let yaml: Value = serde_yaml::from_str(content)?;
    collect_from_yaml(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptor_git_url() {
        let descriptor = "cjk-slug@git@github.com:daangn/cjk-slug.git#commit=de5d97557a09ad61ae6ac48b1258b67d304660f0";
        let descriptor = PackageDescriptor::try_from(descriptor);
        assert_eq!(
            descriptor,
            Ok(PackageDescriptor::Git {
                ident: "cjk-slug".into(),
                url: "git@github.com:daangn/cjk-slug.git".into(),
                commit_hash: "de5d97557a09ad61ae6ac48b1258b67d304660f0".into(),
            }),
        );
    }

    #[test]
    fn test_descriptor_github_url() {
        let descriptor = "cjk-slug@https://github.com/daangn/cjk-slug.git#commit=de5d97557a09ad61ae6ac48b1258b67d304660f0";
        let descriptor = PackageDescriptor::try_from(descriptor);
        assert_eq!(
            descriptor,
            Ok(PackageDescriptor::Git {
                ident: "cjk-slug".into(),
                url: "https://github.com/daangn/cjk-slug.git".into(),
                commit_hash: "de5d97557a09ad61ae6ac48b1258b67d304660f0".into(),
            }),
        );
    }

    #[test]
    fn test_descriptor_private_registry() {
        let descriptor = "@fortawesome/pro-solid-svg-icons@npm:6.4.0::__archiveUrl=https%3A%2F%2Fnpm.fontawesome.com%2F%40fortawesome%2Fpro-solid-svg-icons%2F-%2F6.4.0%2Fpro-solid-svg-icons-6.4.0.tgz";
        let descriptor = PackageDescriptor::try_from(descriptor);
        assert_eq!(
            descriptor,
            Ok(PackageDescriptor::Regular {
                ident: "@fortawesome/pro-solid-svg-icons".into(),
                range: PackageRange {
                    protocol: "npm:".into(),
                    selector: "6.4.0".into(),
                    source: None,
                    bindings: Some(HashMap::from([
                        ("__archiveUrl".to_owned(), "https://npm.fontawesome.com/@fortawesome/pro-solid-svg-icons/-/6.4.0/pro-solid-svg-icons-6.4.0.tgz".to_owned())
                    ])),
                }
            }),
        );
    }
}
