use anyhow::Result;
use fossgraph_core::dependency::Dependency;
use fossgraph_fetcher::fetch;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<()> {
    let dep = Dependency::Npm {
        name: "@urlpack/json".into(),
        version: "1.1.0".into(),
    };
    let source = fetch(&dep).await?;
    let bytes = source.into_inner();

    let mut file = tokio::fs::File::create("test.zip").await?;
    file.write_all(bytes.as_ref()).await?;

    Ok(())
}
