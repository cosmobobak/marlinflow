use std::{thread::ScopedJoinHandle, sync::atomic::AtomicU64};

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
    threads: Option<usize>,
}

unsafe fn mmap_into_slice_mut_with_lifetime<T>(mmap: &mut MmapMut) -> &mut [T] {
    let len = mmap.len() / std::mem::size_of::<T>();
    std::slice::from_raw_parts_mut(mmap.as_mut_ptr() as *mut T, len)
}

pub fn run(options: Options) -> anyhow::Result<()> {
    // Initialize tablebases
    tablebases::probe::init(
        options
            .tb_path
            .to_str()
            .with_context(|| "Failed to convert tb_path to str")?,
    );
    println!("Highest Syzygy cardinality found: {}", tablebases::probe::get_max_pieces_count());
    // Open the dataset
    let dataset = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(options.dataset)
        .with_context(|| "Failed to open dataset")?;
    // mmap the dataset
    let mut mmap =
        unsafe { memmap::MmapMut::map_mut(&dataset).with_context(|| "Failed to mmap dataset")? };
    // Get a slice of the dataset
    let positions = unsafe { mmap_into_slice_mut_with_lifetime::<PackedBoard>(&mut mmap) };

    // Determine threads to split work over
    let max_threads = num_cpus::get();
    let threads = options.threads.map(|t| t.min(max_threads)).unwrap_or(max_threads);
    let positions_processed = AtomicU64::new(0);
    let processed_ref = &positions_processed;
    let total_positions = positions.len() as u64;
    let digit_width = total_positions.to_string().len();

    // Split the work over the threads
    std::thread::scope(|scope| {
        let mut handles: Vec<ScopedJoinHandle<'_, Result<(), anyhow::Error>>> =
            Vec::with_capacity(threads);
        for (i, chunk) in positions.chunks_mut((positions.len() + threads - 1) / threads).enumerate() {
            // Spawn a thread
            handles.push(scope.spawn(move || {
                if i == 0 {
                    print!("Rescoring positions: {:w$}/{} (  0.00%)", 0, total_positions, w = digit_width);
                }
                for (p_idx, position) in chunk.iter_mut().enumerate() {
                    // unpack the position
                    let (board, eval, wdl, extra) = position
                        .unpack()
                        .with_context(|| "Failed to unpack position")?;
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
                    // update progress in batches -
                    // all threads add to the counter, but only one prints.
                    if p_idx % 1024 == 0 {
                        processed_ref.fetch_add(1024, std::sync::atomic::Ordering::Relaxed);
                        if i == 0 {
                            // we're the main thread, print progress
                            let processed = processed_ref.load(std::sync::atomic::Ordering::Relaxed);
                            let percent = (processed as f64 / total_positions as f64) * 100.0;
                            print!("\rRescoring positions: {:w$}/{} ({:6.2}%)", processed, total_positions, percent, w = digit_width);
                        }
                    }
                }
                Ok(())
            }));
        }
        // Wait for all threads to finish
        for handle in handles {
            handle.join().unwrap()?;
        }
        Ok::<(), anyhow::Error>(())
    })?;
    // update the progress bar to 100%
    println!("\rRescoring positions: {}/{} (100.00%)", total_positions, total_positions);

    Ok(())
}
