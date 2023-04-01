use structopt::StructOpt;

mod convert;
mod interleave;
mod shuffle;
mod txt_to_data;

#[derive(StructOpt)]
/// Shuffle a dataset
pub struct CountOptions {
    dataset: std::path::PathBuf,
}

#[derive(StructOpt)]
pub enum Options {
    Convert(convert::Options),
    Shuffle(shuffle::Options),
    Interleave(interleave::Options),
    TxtToData(txt_to_data::Options),
    Count(CountOptions),
}

fn main() {
    match Options::from_args() {
        Options::Convert(options) => convert::run(options),
        Options::Shuffle(options) => shuffle::run(options).unwrap(),
        Options::Interleave(options) => interleave::run(options).unwrap(),
        Options::TxtToData(options) => txt_to_data::run(options).unwrap(),
        Options::Count(options) => {
            let mut dataset = std::fs::File::open(options.dataset).unwrap();
            let positions = std::io::Seek::seek(&mut dataset, std::io::SeekFrom::End(0)).unwrap()
                / std::mem::size_of::<marlinformat::PackedBoard>() as u64;
            println!("{} positions", positions);
        }
    }
}
