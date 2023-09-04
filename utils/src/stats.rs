use std::io::{Read, Seek, SeekFrom, Write};

use bytemuck::Zeroable;
use marlinformat::PackedBoard;
use structopt::StructOpt;

use cozy_chess::{Color, Square};

#[cfg(feature = "syzygy")]
use crate::tablebases;

#[derive(StructOpt)]
/// Report statistics about a dataset
pub struct Options {
    dataset: std::path::PathBuf,
    #[cfg(feature = "syzygy")]
    #[structopt(long)]
    tb_path: Option<std::path::PathBuf>,
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

pub fn run(options: Options) -> anyhow::Result<()> {
    #[cfg(feature = "syzygy")]
    if let Some(tb_path) = &options.tb_path {
        tablebases::probe::init(tb_path.to_str().unwrap());
        println!("[WARNING] Syzygy probing enabled. This will be slooooow.");
    }

    let mut dataset = std::fs::File::open(options.dataset)?;
    let size_bytes = dataset.seek(SeekFrom::End(0))?;
    dataset.seek(SeekFrom::Start(0))?;
    let mut count = size_bytes / std::mem::size_of::<PackedBoard>() as u64;
    let positions = count;
    let mut reader = std::io::BufReader::new(dataset);
    
    let mut stats = Stats::default();

    let is_significantly_incongruent = |cp_eval: i32, wdl: u8| -> bool {
        let wdl = wdl as i32 - 1; // 1 = win, 0 = draw, -1 = loss
        match () {
            _ if cp_eval > 200 && wdl == -1 => true, // winning eval but loss
            _ if cp_eval < -200 && wdl == 1 => true, // losing eval but win
            _ if cp_eval.abs() > 400 && wdl == 0 => true, // large eval but draw
            _ => false,
        }
    };

    let start_time = std::time::Instant::now();

    while count != 0 {
        let mut value = PackedBoard::zeroed();
        reader.read_exact(bytemuck::bytes_of_mut(&mut value))?;
        let (board, eval, wdl, _extra) = value.unpack().expect("invalid board");
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
        count -= 1;
        #[cfg(feature = "syzygy")]
        if options.tb_path.is_some() {
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
        if count & 0xFFFFF == 0 {
            let progress = positions - count;
            let proportion = progress as f64 / positions as f64;
            let elapsed = start_time.elapsed();
            let elapsed = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 * 1e-9;
            let time_per_position = elapsed / progress as f64;
            let time_left = time_per_position * count as f64;
            let minutes = (time_left / 60.0) as u64;
            let seconds = (time_left - minutes as f64 * 60.0) as u64;
            print!("\r\x1B[K{progress:12}/{positions} ({:4.1}%), estimated time left: {minutes}m {seconds}s", proportion * 100.0);
            let _ = std::io::stdout().flush();
        }
    }
    println!();
    println!("{} positions", positions);

    println!("Pieces on board:");
    let mean = stats.pieces_on_board
        .iter()
        .enumerate()
        .map(|(i, v)| i as u64 * v)
        .sum::<u64>() as f64
        / positions as f64;
    println!("  Mean: {:.2}", mean);
    let mut for_sorting = stats.pieces_on_board
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
        let pcnt = *v as f64 / positions as f64 * 100.0;
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

    let mean_mvcnt = stats.movecount
        .iter()
        .enumerate()
        .map(|(i, v)| i as u64 * v)
        .sum::<u64>() as f64
        / positions as f64;
    println!("Mean movecount: {:.2}", mean_mvcnt);

    println!("Win/draw/loss:");
    println!(
        "  Win:  {:.2}%",
        stats.win_draw_loss[0] as f64 / positions as f64 * 100.0
    );
    println!(
        "  Draw: {:.2}%",
        stats.win_draw_loss[1] as f64 / positions as f64 * 100.0
    );
    println!(
        "  Loss: {:.2}%",
        stats.win_draw_loss[2] as f64 / positions as f64 * 100.0
    );

    println!("White king positions:");
    let (idx, max) = stats.white_king_positions
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| **v)
        .unwrap();
    let idx_sq = Square::index(idx);
    let pcnt = *max as f64 / positions as f64 * 100.0;
    println!("  Most: {} on {} ({:.3}%)", max, idx_sq, pcnt);
    let (idx, min) = stats.white_king_positions
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| **v)
        .unwrap();
    let idx_sq = Square::index(idx);
    let pcnt = *min as f64 / positions as f64 * 100.0;
    println!("  Least: {} on {} ({:.3}%)", min, idx_sq, pcnt);
    println!("Black king positions:");
    let (idx, max) = stats.black_king_positions
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| **v)
        .unwrap();
    let idx_sq = Square::index(idx);
    let pcnt = *max as f64 / positions as f64 * 100.0;
    println!("  Most: {} on {} ({:.3}%)", max, idx_sq, pcnt);
    let (idx, min) = stats.black_king_positions
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| **v)
        .unwrap();
    let idx_sq = Square::index(idx);
    let pcnt = *min as f64 / positions as f64 * 100.0;
    println!("  Least: {} on {} ({:.3}%)", min, idx_sq, pcnt);

    println!("Data health metrics:");
    println!(
        "  Number of positions where eval and WDL are significantly incongruent: {} ({:.3}%)",
        stats.incongruent,
        stats.incongruent as f64 / positions as f64 * 100.0
    );
    println!(
        "  Number of positions where eval is extremely large: {} ({:.3}%)",
        stats.extremely_large_eval,
        stats.extremely_large_eval as f64 / positions as f64 * 100.0
    );
    #[cfg(feature = "syzygy")]
    {
        if options.tb_path.is_some() {
            println!(
                "  Number of Syzygy tablebase hits: {} ({:.3}%)",
                stats.tb_hits,
                stats.tb_hits as f64 / positions as f64 * 100.0
            );
            println!("  Number of positions where Syzygy tablebase disagrees with game outcome: {} ({:.3}%)", stats.incorrect_syzygy, stats.incorrect_syzygy as f64 / positions as f64 * 100.0);
            println!("  Fraction of hits where Syzygy tablebase disagrees with game outcome: {} / {} ({:.3}%)", stats.incorrect_syzygy, stats.tb_hits, stats.incorrect_syzygy as f64 / stats.tb_hits as f64 * 100.0);
        } else {
            println!(
                "  Syzygy tablebase path not specified, no Syzygy tablebase metrics available"
            );
        }
    }

    Ok(())
}
