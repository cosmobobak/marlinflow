use std::{sync::atomic::AtomicU64, thread::ScopedJoinHandle};

use anyhow::Context;
use marlinformat::PackedBoard;
use memmap::Mmap;
use structopt::StructOpt;

use cozy_chess::{Color, Square};

use crate::tablebases;
#[cfg(feature = "syzygy")]
use crate::tablebases;

#[derive(StructOpt)]
/// Report statistics about a dataset
pub struct Options {
    dataset: std::path::PathBuf,
    tb_path: Option<std::path::PathBuf>,
    threads: Option<usize>,
}

struct Stats {
    white_king_positions: [u64; 64],
    black_king_positions: [u64; 64],
    pieces_on_board: [u64; 33],
    pieces_on_board_by_type: [u64; 6],
    movecount: [u64; 2048],
    win_draw_loss: [u64; 3],
    incongruent: u64,
    extremely_large_eval: u64,
    incorrect_syzygy: u64,
    tb_hits: u64,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            white_king_positions: [0; 64],
            black_king_positions: [0; 64],
            pieces_on_board: [0; 33],
            pieces_on_board_by_type: Default::default(),
            movecount: [0; 2048],
            win_draw_loss: Default::default(),
            incongruent: Default::default(),
            extremely_large_eval: Default::default(),
            incorrect_syzygy: Default::default(),
            tb_hits: Default::default(),
        }
    }
}

impl std::ops::AddAssign for Stats {
    fn add_assign(&mut self, rhs: Self) {
        self.white_king_positions
            .iter_mut()
            .zip(rhs.white_king_positions.iter())
            .for_each(|(a, b)| *a += b);
        self.black_king_positions
            .iter_mut()
            .zip(rhs.black_king_positions.iter())
            .for_each(|(a, b)| *a += b);
        self.pieces_on_board
            .iter_mut()
            .zip(rhs.pieces_on_board.iter())
            .for_each(|(a, b)| *a += b);
        self.pieces_on_board_by_type
            .iter_mut()
            .zip(rhs.pieces_on_board_by_type.iter())
            .for_each(|(a, b)| *a += b);
        self.movecount
            .iter_mut()
            .zip(rhs.movecount.iter())
            .for_each(|(a, b)| *a += b);
        self.win_draw_loss
            .iter_mut()
            .zip(rhs.win_draw_loss.iter())
            .for_each(|(a, b)| *a += b);
        self.incongruent += rhs.incongruent;
        self.extremely_large_eval += rhs.extremely_large_eval;
        self.incorrect_syzygy += rhs.incorrect_syzygy;
        self.tb_hits += rhs.tb_hits;
    }
}

unsafe fn mmap_into_slice_with_lifetime<T>(mmap: &Mmap) -> &[T] {
    let len = mmap.len() / std::mem::size_of::<T>();
    std::slice::from_raw_parts(mmap.as_ptr() as *const T, len)
}

pub fn run(options: Options) -> anyhow::Result<()> {
    if let Some(tb_path) = &options.tb_path {
        if cfg!(not(feature = "syzygy")) {
            println!("[WARNING] Syzygy probing requested but not enabled. Ignoring.");
        } else {
            #[cfg(feature = "syzygy")]
            tablebases::probe::init(tb_path.to_str().unwrap());
            println!("[WARNING] Syzygy probing enabled. This will be slooooow.");
            #[cfg(not(feature = "syzygy"))]
            let _ = tb_path;
        }
    }

    // Open the dataset
    let dataset = std::fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(options.dataset)
        .with_context(|| "Failed to open dataset")?;
    // mmap the dataset
    let mmap = unsafe { memmap::Mmap::map(&dataset).with_context(|| "Failed to mmap dataset")? };
    // Get a slice of the dataset
    let positions = unsafe { mmap_into_slice_with_lifetime::<PackedBoard>(&mmap) };

    let count = positions.len();

    let is_significantly_incongruent = |cp_eval: i32, wdl: u8| -> bool {
        let wdl = wdl as i32 - 1; // 1 = win, 0 = draw, -1 = loss
        match () {
            _ if cp_eval > 200 && wdl == -1 => true, // winning eval but loss
            _ if cp_eval < -200 && wdl == 1 => true, // losing eval but win
            _ if cp_eval.abs() > 400 && wdl == 0 => true, // large eval but draw
            _ => false,
        }
    };

    // Determine threads to split work over
    let max_threads = num_cpus::get();
    let threads = options
        .threads
        .map(|t| t.min(max_threads))
        .unwrap_or(max_threads);
    let positions_processed = AtomicU64::new(0);
    let processed_ref = &positions_processed;
    let total_positions = positions.len() as u64;
    let digit_width = total_positions.to_string().len();

    let tb_path = options.tb_path.as_ref();

    let stats = std::thread::scope(|scope| {
        let mut handles: Vec<ScopedJoinHandle<'_, Result<Stats, anyhow::Error>>> =
            Vec::with_capacity(threads);
        for (i, chunk) in positions
            .chunks((positions.len() + threads - 1) / threads)
            .enumerate()
        {
            // Spawn a thread
            handles.push(scope.spawn(move || {
                let mut stats = Stats::default();
                if i == 0 {
                    print!(
                        "Rescoring positions: {:w$}/{} (  0.00%)",
                        0,
                        total_positions,
                        w = digit_width
                    );
                }
                for (p_idx, position) in chunk.iter().enumerate() {
                    // unpack the position
                    let (board, eval, wdl, _extra) = position
                        .unpack()
                        .with_context(|| "Failed to unpack position")?;
                    let wk_pos = board.king(Color::White);
                    let bk_pos = board.king(Color::Black);
                    stats.white_king_positions[wk_pos as usize] += 1;
                    stats.black_king_positions[bk_pos as usize] += 1;
                    let piece_count = board.occupied().len();
                    stats.pieces_on_board[piece_count] += 1;
                    for sq in board.occupied() {
                        let ty = board.piece_on(sq).unwrap();
                        stats.pieces_on_board_by_type[ty as usize] += 1;
                    }
                    stats.movecount[board.fullmove_number() as usize] += 1;
                    stats.win_draw_loss[wdl as usize] += 1;
                    stats.incongruent += is_significantly_incongruent(i32::from(eval), wdl) as u64;
                    stats.extremely_large_eval += (eval.abs() > i16::MAX - 200) as u64;

                    if tb_path.is_some() {
                        if let Some(tb_wdl) = tablebases::probe::get_wdl_white(&board) {
                            stats.tb_hits += 1;
                            let tb_wdl = match tb_wdl {
                                tablebases::probe::WDL::Win => 1,
                                tablebases::probe::WDL::Draw => 0,
                                tablebases::probe::WDL::Loss => -1,
                            };
                            if tb_wdl != wdl as i8 - 1 {
                                stats.incorrect_syzygy += 1;
                            }
                        }
                    }
                    // update progress in batches -
                    // all threads add to the counter, but only one prints.
                    if p_idx % 1024 == 0 {
                        processed_ref.fetch_add(1024, std::sync::atomic::Ordering::Relaxed);
                        if i == 0 {
                            // we're the main thread, print progress
                            let processed =
                                processed_ref.load(std::sync::atomic::Ordering::Relaxed);
                            let percent = (processed as f64 / total_positions as f64) * 100.0;
                            print!(
                                "\rRescoring positions: {:w$}/{} ({:6.2}%)",
                                processed,
                                total_positions,
                                percent,
                                w = digit_width
                            );
                        }
                    }
                }

                Ok(stats)
            }));
        }
        // Wait for all threads to finish
        let mut stats_accumulator = Stats::default();
        for handle in handles {
            let s = handle.join().unwrap()?;
            stats_accumulator += s;
        }
        Ok::<_, anyhow::Error>(stats_accumulator)
    })?;
    // update the progress bar to 100%
    println!("\rRescoring positions: {}/{} (100.00%)", total_positions, total_positions);

    println!();
    println!("{} positions", count);

    println!("Pieces on board:");
    let mean = stats
        .pieces_on_board
        .iter()
        .enumerate()
        .map(|(i, v)| i as u64 * v)
        .sum::<u64>() as f64
        / count as f64;
    println!("  Mean: {:.2}", mean);
    let mut for_sorting = stats
        .pieces_on_board
        .iter()
        .copied()
        .enumerate()
        .collect::<Vec<_>>();
    for_sorting.sort_by_key(|(_, v)| *v);
    let median = for_sorting[for_sorting.len() / 2];
    println!("  Median: {}", median.0);
    println!("  Distribution:");
    let mut left_column = Vec::with_capacity(15);
    let mut right_column = Vec::with_capacity(15);
    for (i, v) in stats.pieces_on_board.iter().enumerate().skip(3) {
        let pcnt = *v as f64 / count as f64 * 100.0;
        let s = format!("{:2}: {} ({:.1}%)", i, v, pcnt);
        if i < 17 {
            left_column.push(s);
        } else {
            right_column.push(s);
        }
    }
    for (l, r) in left_column.into_iter().zip(right_column.into_iter()) {
        println!("  {:<40}{}", l, r);
    }

    let mean_mvcnt = stats
        .movecount
        .iter()
        .enumerate()
        .map(|(i, v)| i as u64 * v)
        .sum::<u64>() as f64
        / count as f64;
    println!("Mean movecount: {:.2}", mean_mvcnt);

    println!("Win/draw/loss:");
    println!(
        "  Win:  {:.2}%",
        stats.win_draw_loss[0] as f64 / count as f64 * 100.0
    );
    println!(
        "  Draw: {:.2}%",
        stats.win_draw_loss[1] as f64 / count as f64 * 100.0
    );
    println!(
        "  Loss: {:.2}%",
        stats.win_draw_loss[2] as f64 / count as f64 * 100.0
    );

    println!("White king positions:");
    let (idx, max) = stats
        .white_king_positions
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| **v)
        .unwrap();
    let idx_sq = Square::index(idx);
    let pcnt = *max as f64 / count as f64 * 100.0;
    println!("  Most: {} on {} ({:.3}%)", max, idx_sq, pcnt);
    let (idx, min) = stats
        .white_king_positions
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| **v)
        .unwrap();
    let idx_sq = Square::index(idx);
    let pcnt = *min as f64 / count as f64 * 100.0;
    println!("  Least: {} on {} ({:.3}%)", min, idx_sq, pcnt);
    println!("Black king positions:");
    let (idx, max) = stats
        .black_king_positions
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| **v)
        .unwrap();
    let idx_sq = Square::index(idx);
    let pcnt = *max as f64 / count as f64 * 100.0;
    println!("  Most: {} on {} ({:.3}%)", max, idx_sq, pcnt);
    let (idx, min) = stats
        .black_king_positions
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| **v)
        .unwrap();
    let idx_sq = Square::index(idx);
    let pcnt = *min as f64 / count as f64 * 100.0;
    println!("  Least: {} on {} ({:.3}%)", min, idx_sq, pcnt);

    println!("Data health metrics:");
    println!(
        "  Number of positions where eval and WDL are significantly incongruent: {} ({:.3}%)",
        stats.incongruent,
        stats.incongruent as f64 / count as f64 * 100.0
    );
    println!(
        "  Number of positions where eval is extremely large: {} ({:.3}%)",
        stats.extremely_large_eval,
        stats.extremely_large_eval as f64 / count as f64 * 100.0
    );
    #[cfg(feature = "syzygy")]
    {
        if options.tb_path.is_some() {
            println!(
                "  Number of Syzygy tablebase hits: {} ({:.3}%)",
                stats.tb_hits,
                stats.tb_hits as f64 / count as f64 * 100.0
            );
            println!("  Number of positions where Syzygy tablebase disagrees with game outcome: {} ({:.3}%)", stats.incorrect_syzygy, stats.incorrect_syzygy as f64 / count as f64 * 100.0);
            println!("  Fraction of hits where Syzygy tablebase disagrees with game outcome: {} / {} ({:.3}%)", stats.incorrect_syzygy, stats.tb_hits, stats.incorrect_syzygy as f64 / stats.tb_hits as f64 * 100.0);
        } else {
            println!(
                "  Syzygy tablebase path not specified, no Syzygy tablebase metrics available"
            );
        }
    }

    Ok(())
}
