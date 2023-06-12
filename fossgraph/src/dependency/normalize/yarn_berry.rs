use std::collections::{HashMap, HashSet};

use crate::dependency::Dependency;

use fancy_regex::Regex;
use lazy_static::lazy_static;
use percent_encoding::percent_decode_str;
use serde_yaml::Value;
use url::Url;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Couldn't parse the lockfile.\n{message}")]
    InvalidLockfileFormat { message: String },

    #[error("Unsupported resolution: {resolution}")]
    UnsupportedResolution { resolution: String },
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
//    Some eceptions here is `git` and `github` protocols have other forms.
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
        if range.starts_with("git@") {
            // Note: Yarn serialize it as non-compatible with git protocol
            let (hostname, other) = range.split_once("/").unwrap();
            let range = vec![hostname, other].join(":");
            if let Some((url, commit_hash)) = range.split_once("#commit=") {
                Ok(Self::Git {
                    ident: ident.into(),
                    url: url.into(),
                    commit_hash: commit_hash.into(),
                })
            } else {
                Err(Error::invalid_descriptor(value))
            }
        } else if range.starts_with("https://github.com/") {
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

impl TryFrom<String> for PackageDescriptor {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
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

fn normalize_single_resolution(resolution: &str) -> Result<Dependency, Error> {
    let descriptor = PackageDescriptor::try_from(resolution)?;
    match descriptor {
        PackageDescriptor::Git {
            url, commit_hash, ..
        } => Ok(Dependency::Git {
            url,
            head: Some(commit_hash),
        }),
        PackageDescriptor::Regular { ident, range } => {
            match range.protocol.as_str() {
                "npm:" => {
                    let archive_url = range
                        .bindings
                        .map(|bindings| {
                            bindings
                                .get("__archiveUrl")
                                .map(|archive_url| archive_url.clone())
                        })
                        .flatten();
                    if archive_url.is_none() {
                        Ok(Dependency::Npm {
                            name: ident,
                            version: range.selector,
                        })
                    } else {
                        // private/custom registry is not supported
                        Err(Error::UnsupportedResolution {
                            resolution: resolution.into(),
                        })
                    }
                }
                "patch:" => match percent_decode_str(range.protocol.as_str()).decode_utf8() {
                    Ok(nested_descriptor) => {
                        let inner_resolution = nested_descriptor.to_string();
                        normalize_single_resolution(inner_resolution.as_str())
                    }
                    Err(_) => Err(Error::invalid_format()),
                },
                _ => Err(Error::UnsupportedResolution {
                    resolution: resolution.into(),
                }),
            }
        }
    }
}

fn normalize_yaml(value: Value) -> Result<HashSet<Dependency>, Error> {
    if let Some(map) = value.as_mapping() {
        let mut deps: HashSet<Dependency> = HashSet::new();

        let mut iter = map.iter();
        let (_key, _value) = iter.next().unwrap(); // skip metadata
        for (_key, value) in iter {
            let resolution = value
                .as_mapping()
                .and_then(|map| map.get("resolution"))
                .and_then(|value| value.as_str())
                .ok_or_else(|| Error::invalid_format())?;
            match normalize_single_resolution(resolution) {
                Ok(dependency) => {
                    deps.insert(dependency);
                }
                Err(Error::UnsupportedResolution { .. }) => {
                    // noop
                }
                Err(error) => {
                    return Err(error);
                }
            }
        }
        Ok(deps)
    } else {
        Err(Error::invalid_format())
    }
}

pub fn normalize(value: &str) -> Result<HashSet<Dependency>, Error> {
    let yaml: Value = serde_yaml::from_str(value)?;
    normalize_yaml(yaml)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    #[test]
    fn test_normalize() {
        let lockfile = indoc! {r#"
          # This file is generated by running "yarn install" inside your project.
          # Manual changes might be lost - proceed with caution!
          
          __metadata:
            version: 6
            cacheKey: 8
          
          "@fortawesome/fontawesome-common-types@npm:6.4.0":
            version: 6.4.0
            resolution: "@fortawesome/fontawesome-common-types@npm:6.4.0::__archiveUrl=https%3A%2F%2Fnpm.fontawesome.com%2F%40fortawesome%2Ffontawesome-common-types%2F-%2F6.4.0%2Ffontawesome-common-types-6.4.0.tgz"
            checksum: a9b79136caa615352bd921cfe2710516321b402cd76c3f0ae68e579a7e3d7645c5a5c0ecd7516c0b207adeeffd1d2174978638d8c0d3c8c937d66fca4f2ff556
            languageName: node
            linkType: hard
          
          "@fortawesome/pro-solid-svg-icons@npm:^6.4.0":
            version: 6.4.0
            resolution: "@fortawesome/pro-solid-svg-icons@npm:6.4.0::__archiveUrl=https%3A%2F%2Fnpm.fontawesome.com%2F%40fortawesome%2Fpro-solid-svg-icons%2F-%2F6.4.0%2Fpro-solid-svg-icons-6.4.0.tgz"
            dependencies:
              "@fortawesome/fontawesome-common-types": 6.4.0
            checksum: f30e6573528355c6238ba96801bf2eaa9b0221b5e2d70e99b0874dd946e8e8bd2e193ff8cd6bc966d741264162fca1f606b458b611bd262e61d49b6679d44b3a
            languageName: node
            linkType: hard
          
          "berry-lock@workspace:.":
            version: 0.0.0-use.local
            resolution: "berry-lock@workspace:."
            dependencies:
              "@fortawesome/pro-solid-svg-icons": ^6.4.0
              cjk-slug: git@github.com/daangn/cjk-slug.git
              cjk-slug-github: daangn/cjk-slug
              cjk-slug-github-2: "github:daangn/cjk-slug"
              cjk-slug-github-3: "git+https://github.com/daangn/cjk-slug.git"
              lru-cache: ^9.1.2
              semver: ^7.5.1
            languageName: unknown
            linkType: soft
          
          "cjk-slug-github-2@github:daangn/cjk-slug":
            version: 0.3.1
            resolution: "cjk-slug-github-2@https://github.com/daangn/cjk-slug.git#commit=de5d97557a09ad61ae6ac48b1258b67d304660f0"
            dependencies:
              normalize-cjk: ^0.4.0
            checksum: 770c0ad59f2780ba04655f598b2e155306ed4a79e6061926b2c0236038dadacadbc7c7e97b12e003c63882eaf3ef17aae6373f60a70dcb0110b578cd19bcf935
            languageName: node
            linkType: hard
          
          "cjk-slug-github-3@git+https://github.com/daangn/cjk-slug.git":
            version: 0.3.1
            resolution: "cjk-slug-github-3@https://github.com/daangn/cjk-slug.git#commit=de5d97557a09ad61ae6ac48b1258b67d304660f0"
            dependencies:
              normalize-cjk: ^0.4.0
            checksum: 22125c84772553adb317a2ad89b36b3c736a6e3736a921bd7be41352e0a8760d0e65004ed7386a2921b99344e74d80e6811321048fb3a9727950faa88259e768
            languageName: node
            linkType: hard
          
          cjk-slug-github@daangn/cjk-slug:
            version: 0.3.1
            resolution: "cjk-slug-github@https://github.com/daangn/cjk-slug.git#commit=de5d97557a09ad61ae6ac48b1258b67d304660f0"
            dependencies:
              normalize-cjk: ^0.4.0
            checksum: b2cbaa844bc1cb42bcbfd57687d46974529f72d22ab25799f1ccc0ae8c7dc3d99fb6ed251e580f8302d505fa8ea4e6fde62a11d6617c03ed08f1561fc83aa81a
            languageName: node
            linkType: hard
          
          cjk-slug@git@github.com/daangn/cjk-slug.git:
            version: 0.3.1
            resolution: "cjk-slug@git@github.com/daangn/cjk-slug.git#commit=de5d97557a09ad61ae6ac48b1258b67d304660f0"
            dependencies:
              normalize-cjk: ^0.4.0
            checksum: a5d510474265944e569a89a36a893ed30bc676c41118b15394237de553b7172c17dcff3be8734a0d708a221e11e3d078bb5685c6820550c4784e4a84dc7b54d3
            languageName: node
            linkType: hard
          
          "lru-cache@npm:^6.0.0":
            version: 6.0.0
            resolution: "lru-cache@npm:6.0.0"
            dependencies:
              yallist: ^4.0.0
            checksum: f97f499f898f23e4585742138a22f22526254fdba6d75d41a1c2526b3b6cc5747ef59c5612ba7375f42aca4f8461950e925ba08c991ead0651b4918b7c978297
            languageName: node
            linkType: hard
          
          "lru-cache@npm:^9.1.2":
            version: 9.1.2
            resolution: "lru-cache@npm:9.1.2"
            checksum: d3415634be3908909081fc4c56371a8d562d9081eba70543d86871b978702fffd0e9e362b83921b27a29ae2b37b90f55675aad770a54ac83bb3e4de5049d4b15
            languageName: node
            linkType: hard
          
          "normalize-cjk@npm:^0.4.0":
            version: 0.4.0
            resolution: "normalize-cjk@npm:0.4.0"
            checksum: 424059f5b226df99609843788ba80d7727ed0d16821d029c4e800d69aee2e64bd10ba6956fd91eaa99e264ad443ce55e502cdeaa3cfe38222063b1734b106941
            languageName: node
            linkType: hard
          
          "semver@npm:^7.5.1":
            version: 7.5.1
            resolution: "semver@npm:7.5.1"
            dependencies:
              lru-cache: ^6.0.0
            bin:
              semver: bin/semver.js
            checksum: d16dbedad53c65b086f79524b9ef766bf38670b2395bdad5c957f824dcc566b624988013564f4812bcace3f9d405355c3635e2007396a39d1bffc71cfec4a2fc
            languageName: node
            linkType: hard
          
          "yallist@npm:^4.0.0":
            version: 4.0.0
            resolution: "yallist@npm:4.0.0"
            checksum: 343617202af32df2a15a3be36a5a8c0c8545208f3d3dfbc6bb7c3e3b7e8c6f8e7485432e4f3b88da3031a6e20afa7c711eded32ddfb122896ac5d914e75848d5
            languageName: node
            linkType: hard 
        "#};

        let result = normalize(lockfile).unwrap();
        assert_eq!(
            result,
            HashSet::from([
                Dependency::Npm {
                    name: "normalize-cjk".into(),
                    version: "0.4.0".into(),
                },
                Dependency::Npm {
                    name: "semver".into(),
                    version: "7.5.1".into(),
                },
                Dependency::Npm {
                    name: "lru-cache".into(),
                    version: "6.0.0".into(),
                },
                Dependency::Npm {
                    name: "lru-cache".into(),
                    version: "9.1.2".into(),
                },
                Dependency::Npm {
                    name: "yallist".into(),
                    version: "4.0.0".into(),
                },
                Dependency::Git {
                    url: "https://github.com/daangn/cjk-slug.git".into(),
                    head: Some("de5d97557a09ad61ae6ac48b1258b67d304660f0".into()),
                },
                // TODO: deduplicate it by canonicalizing it
                Dependency::Git {
                    url: "git@github.com:daangn/cjk-slug.git".into(),
                    head: Some("de5d97557a09ad61ae6ac48b1258b67d304660f0".into()),
                },
            ]),
        );
    }

    #[test]
    fn test_descriptor_git_url() {
        let descriptor = "cjk-slug@git@github.com/daangn/cjk-slug.git#commit=de5d97557a09ad61ae6ac48b1258b67d304660f0";
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
