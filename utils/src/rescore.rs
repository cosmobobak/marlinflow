
use anyhow::Context;
use marlinformat::PackedBoard;
use memmap::MmapMut;
use structopt::StructOpt;

use crate::tablebases;

#[derive(StructOpt)]
/// Scan a dataset and rescore positions using tablebases
pub struct Options {
    dataset: std::path::PathBuf,
    tb_path: std::path::PathBuf,
}

unsafe fn mmap_into_slice_mut_with_lifetime<T>(mmap: &mut MmapMut) -> &mut [T] {
    let len = mmap.len() / std::mem::size_of::<T>();
    std::slice::from_raw_parts_mut(
        mmap.as_mut_ptr() as *mut T,
        len,
    )
}

pub fn run(options: Options) -> anyhow::Result<()> {
    // Initialize tablebases
    tablebases::probe::init(
        options.tb_path.to_str().with_context(|| "Failed to convert tb_path to str")?,
    );
    // Open the dataset
    let dataset = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(options.dataset)
        .with_context(|| "Failed to open dataset")?;
    // mmap the dataset
    let mut mmap = unsafe { 
        memmap::MmapMut::map_mut(&dataset).with_context(|| "Failed to mmap dataset")?
    };
    // Get a slice of the dataset
    let positions = unsafe {
        mmap_into_slice_mut_with_lifetime::<PackedBoard>(&mut mmap)
    };
    for position in positions.iter_mut() {
        // unpack the position
        let (board, eval, wdl, extra) = position.unpack().with_context(|| "Failed to unpack position")?;
        // probe
        if let Some(tb_wdl) = tablebases::probe::get_wdl_white(&board) {
            let tb_wdl = match tb_wdl {
                tablebases::probe::WDL::Win => 2,
                tablebases::probe::WDL::Draw => 1,
                tablebases::probe::WDL::Loss => 0,
            };
            if tb_wdl != wdl {
                // update the position
                *position = PackedBoard::pack(&board, eval, tb_wdl, extra);
            }
        }
    }

    Ok(())
}