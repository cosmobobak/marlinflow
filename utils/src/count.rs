use structopt::StructOpt;

#[derive(StructOpt)]
/// Get a count of positions in a dataset
pub struct Options {
    dataset: std::path::PathBuf,
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let mut dataset = std::fs::File::open(options.dataset)?;
    let positions = std::io::Seek::seek(&mut dataset, std::io::SeekFrom::End(0))?
        / std::mem::size_of::<marlinformat::PackedBoard>() as u64;
    println!("{} positions", positions);
    Ok(())
}