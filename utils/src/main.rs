use structopt::StructOpt;

mod convert;
mod interleave;
mod shuffle;
mod txt_to_data;
mod data_to_txt;
mod count;
mod stats;
mod tablebases;
mod rescore;

#[derive(StructOpt)]
pub enum Options {
    Convert(convert::Options),
    Shuffle(shuffle::Options),
    Interleave(interleave::Options),
    TxtToData(txt_to_data::Options),
    DataToTxt(data_to_txt::Options),
    Count(count::Options),
    Stats(stats::Options),
    Rescore(rescore::Options),
}

fn main() -> anyhow::Result<()> {
    match Options::from_args() {
        Options::Convert(options) => convert::run(options).map_err(|e| e.into()),
        Options::Shuffle(options) => shuffle::run(options),
        Options::Interleave(options) => interleave::run(options),
        Options::TxtToData(options) => txt_to_data::run(options),
        Options::DataToTxt(options) => data_to_txt::run(options),
        Options::Count(options) => count::run(options),
        Options::Stats(options) => stats::run(options),
        Options::Rescore(options) => rescore::run(options),
    }
}
