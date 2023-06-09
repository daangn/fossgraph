pub mod normalize;

// TODO: customize `Hash` trait to canonicalize well-known git URLs
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Dependency {
    Git {
        url: String,
        head: Option<String>, // commit hash, tag, branch
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
