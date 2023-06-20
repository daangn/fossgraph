mod registry;
mod zip_util;

use std::io::Cursor;

use bytes::Bytes;
use fossgraph_core::dependency::Dependency;
use registry::npm::NpmPackage;
use zip::ZipArchive;

#[derive(Debug)]
pub struct Source {
    inner: ZipArchive<Cursor<Bytes>>,
}

impl Source {
    pub fn into_inner(&self) -> Bytes {
        self.inner.to_owned().into_inner().into_inner()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("")]
    NpmError(#[from] registry::npm::Error),

    #[error("")]
    ZipUtilError(#[from] zip_util::Error),
}

pub async fn fetch(dependency: &Dependency) -> Result<Source, Error> {
    match dependency {
        Dependency::Npm { name, version } => {
            let package = NpmPackage {
                name: name.clone(),
                version: version.clone(),
            };
            let mut tar = package.fetch().await?;
            let zip = zip_util::from_tar(&mut tar)?;
            Ok(Source { inner: zip })
        }
        _ => unimplemented!(),
    }
}
