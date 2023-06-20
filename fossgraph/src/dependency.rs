pub mod normalize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Dependency {
    Git {
        url: String,
        head: Option<String>, // commit hash, tag, branch
    },
    GitHub {
        owner: String,
        name: String,
        head: Option<String>,
    },
    Npm {
        name: String,
        version: String,
    },
    CocoaPods {
        name: String,
        version: String,
    },
    Maven {
        group_id: String,
        artifact_id: String,
        version: String,
    },
}

impl Dependency {
    pub fn canonicalize(&self) -> Self {
        match self {
            Self::Git { url, head } => {
                if let Some(substr) = url.strip_prefix("git@github.com:") {
                    let (owner, substr) = substr.split_once('/').unwrap();
                    let (name, _) = substr.split_once(".git").unwrap();
                    return Self::GitHub {
                        owner: owner.into(),
                        name: name.into(),
                        head: head.to_owned(),
                    };
                }

                self.clone()
            }
            _ => self.clone(),
        }
    }
}
