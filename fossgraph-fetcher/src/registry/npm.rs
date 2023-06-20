use bytes::{buf::Reader, Buf, Bytes};
use flate2::read::GzDecoder;
use reqwest::Url;

pub struct NpmPackage {
    pub name: String,
    pub version: String,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to fetch")]
    NetworkError(#[from] reqwest::Error),
}

impl NpmPackage {
    pub fn to_archive_url(&self) -> Url {
        let Self { name, version } = self;
        let url = if let Some((group, name)) = name.split_once('/') {
            format!("https://registry.npmjs.org/{group}/{name}/-/{name}-{version}.tgz")
        } else {
            format!("https://registry.npmjs.org/{name}/-/{name}-{version}.tgz")
        };
        Url::parse(url.as_str()).unwrap()
    }

    pub async fn fetch(&self) -> Result<tar::Archive<GzDecoder<Reader<Bytes>>>, Error> {
        let response = reqwest::get(self.to_archive_url()).await?;
        let body = response.bytes().await?;
        let tarball = GzDecoder::new(body.reader());
        let archive = tar::Archive::new(tarball);
        Ok(archive)
    }
}
