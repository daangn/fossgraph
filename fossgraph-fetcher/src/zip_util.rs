use std::io::{Cursor, Read, Write};

use bytes::{buf::Reader, Bytes};
use flate2::read::GzDecoder;
use zip::{write::FileOptions, ZipArchive, ZipWriter};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("")]
    IoError(#[from] std::io::Error),

    #[error("")]
    ZipError(#[from] zip::result::ZipError),
}

pub fn from_tar(
    tar: &mut tar::Archive<GzDecoder<Reader<Bytes>>>,
) -> Result<ZipArchive<Cursor<Bytes>>, Error> {
    let mut zip_bytes = Vec::new();

    {
        let mut zip_writer = ZipWriter::new(Cursor::new(&mut zip_bytes));

        for entry_result in tar.entries()? {
            let mut entry = entry_result?;
            let path = entry.path()?;
            // let size = entry.header().size()?;
            let mode = entry.header().mode()?;
            let options = FileOptions::default()
                .compression_method(zip::CompressionMethod::Zstd)
                .unix_permissions(mode);

            zip_writer.start_file(path.to_str().unwrap(), options)?;

            let mut buf = Vec::new();
            entry.read(&mut buf)?;
            zip_writer.write(&mut buf)?;
        }

        zip_writer.finish()?;
    }

    let bytes = Bytes::from(zip_bytes);
    let archive = ZipArchive::new(Cursor::new(bytes))?;
    Ok(archive)
}
