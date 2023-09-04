#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    clippy::missing_const_for_fn
)]

use crate::tablebases::bindings::{
    tb_init, tb_probe_root, tb_probe_wdl, TB_BLESSED_LOSS, TB_CURSED_WIN, TB_DRAW, TB_LARGEST,
    TB_LOSS, TB_PROMOTES_BISHOP, TB_PROMOTES_KNIGHT, TB_PROMOTES_QUEEN, TB_PROMOTES_ROOK,
    TB_RESULT_DTZ_MASK, TB_RESULT_DTZ_SHIFT, TB_RESULT_FAILED, TB_RESULT_FROM_MASK,
    TB_RESULT_FROM_SHIFT, TB_RESULT_PROMOTES_MASK, TB_RESULT_PROMOTES_SHIFT, TB_RESULT_TO_MASK,
    TB_RESULT_TO_SHIFT, TB_RESULT_WDL_MASK, TB_RESULT_WDL_SHIFT, TB_WIN,
};
use cozy_chess::{Move, Board, Color, Piece, Square};
use std::ffi::CString;
use std::ptr;

#[allow(clippy::upper_case_acronyms)]
pub enum WDL {
    Win,
    Loss,
    Draw,
}
pub struct WdlDtzResult {
    wdl: WDL,
    dtz: u32,
    best_move: Move,
}

/// Loads Syzygy tablebases stored in `syzygy_path` location.
pub fn init(syzygy_path: &str) {
    #[cfg(feature = "syzygy")]
    unsafe {
        let path = CString::new(syzygy_path).unwrap();
        let res = tb_init(path.as_ptr());
        assert!(res, "Failed to load Syzygy tablebases from {syzygy_path}");
    }
}

/// Gets maximal pieces count supported by loaded Syzygy tablebases. Returns 0 if the feature is disabled.
pub fn get_max_pieces_count() -> u8 {
    #![allow(clippy::cast_possible_truncation)]
    #[cfg(feature = "syzygy")]
    {
        let user_limit = 32;
        let hard_limit = unsafe { TB_LARGEST as u8 };
        std::cmp::min(user_limit, hard_limit)
    }
    #[cfg(not(feature = "syzygy"))]
    0
}

/// Gets WDL (Win-Draw-Loss), DTZ (Distance To Zeroing) and the best move for the position specified in `board`.
/// Returns [None] if data couldn't be obtained or the feature is disabled.
pub fn get_root_wdl_dtz(board: &Board) -> Option<WdlDtzResult> {
    const WHITE: bool = true;
    const BLACK: bool = false;
    #[cfg(feature = "syzygy")]
    unsafe {
        let result = tb_probe_root(
            board.colors(Color::White).0,
            board.colors(Color::Black).0,
            board.pieces(Piece::King).0,
            board.pieces(Piece::Queen).0,
            board.pieces(Piece::Rook).0,
            board.pieces(Piece::Bishop).0,
            board.pieces(Piece::Knight).0,
            board.pieces(Piece::Pawn).0,
            u32::from(board.halfmove_clock()),
            0,
            0,
            board.side_to_move() == Color::White,
            ptr::null_mut(),
        );

        let wdl = (result & TB_RESULT_WDL_MASK) >> TB_RESULT_WDL_SHIFT;
        let wdl = match wdl {
            TB_WIN => WDL::Win,
            TB_LOSS => WDL::Loss,
            _ => WDL::Draw,
        };
        let dtz = (result & TB_RESULT_DTZ_MASK) >> TB_RESULT_DTZ_SHIFT;

        if result == TB_RESULT_FAILED {
            return None;
        }

        let mut moves = [None; 256];
        let mut moves_count = 0;
        board.generate_moves(|set| {
            for m in set.into_iter() {
                moves[moves_count] = Some(m);
                moves_count += 1;
            }
            false
        });
        let moves = &moves[..moves_count];

        let from = Square::index(((result & TB_RESULT_FROM_MASK) >> TB_RESULT_FROM_SHIFT) as usize);
        let to = Square::index(((result & TB_RESULT_TO_MASK) >> TB_RESULT_TO_SHIFT) as usize);
        let promotion = (result & TB_RESULT_PROMOTES_MASK) >> TB_RESULT_PROMOTES_SHIFT;

        let promo_piece_type = match promotion {
            TB_PROMOTES_QUEEN => Some(Piece::Queen),
            TB_PROMOTES_ROOK => Some(Piece::Rook),
            TB_PROMOTES_BISHOP => Some(Piece::Bishop),
            TB_PROMOTES_KNIGHT => Some(Piece::Knight),
            _ => None,
        };

        for m in moves.iter().copied().map(Option::unwrap) {
            if m.from == from
                && m.to == to
                && (promotion == 0 || m.promotion == promo_piece_type)
            {
                return Some(WdlDtzResult {
                    wdl,
                    dtz,
                    best_move: m,
                });
            }
        }

        None
    }
    #[cfg(not(feature = "syzygy"))]
    None
}

/// Gets WDL (Win-Draw-Loss) only for the position specified in `board`.
/// Returns [None] if data couldn't be obtained or the feature is disabled.
fn get_root_wdl(board: &Board) -> Option<WDL> {
    #[cfg(feature = "syzygy")]
    unsafe {
        let result = tb_probe_root(
            board.colors(Color::White).0,
            board.colors(Color::Black).0,
            board.pieces(Piece::King).0,
            board.pieces(Piece::Queen).0,
            board.pieces(Piece::Rook).0,
            board.pieces(Piece::Bishop).0,
            board.pieces(Piece::Knight).0,
            board.pieces(Piece::Pawn).0,
            u32::from(board.halfmove_clock()),
            0,
            0,
            board.side_to_move() == Color::White,
            ptr::null_mut(),
        );

        let wdl = (result & TB_RESULT_WDL_MASK) >> TB_RESULT_WDL_SHIFT;
        let wdl = match wdl {
            TB_WIN => WDL::Win,
            TB_LOSS => WDL::Loss,
            _ => WDL::Draw,
        };

        if result == TB_RESULT_FAILED {
            return None;
        }

        Some(wdl)
    }
    #[cfg(not(feature = "syzygy"))]
    None
}

/// Checks if there's a tablebase move and returns it as [Some], otherwise [None].
pub fn get_tablebase_move(board: &Board) -> Option<(Move, i32)> {
    if board.occupied().len() > get_max_pieces_count() as usize {
        return None;
    }

    let result = get_root_wdl_dtz(board)?;

    let score = match result.wdl {
        WDL::Win => 1,
        WDL::Draw => 0,
        WDL::Loss => -1,
    };

    Some((result.best_move, score))
}

/// Gets the WDL of the position from the perspective of White.
/// Returns [None] if data couldn't be obtained or the feature is disabled.
pub fn get_wdl_white(board: &Board) -> Option<WDL> {
    if board.occupied().len() > get_max_pieces_count() as usize {
        return None;
    }

    let probe_result = get_root_wdl(board)?;

    let stm = board.side_to_move() == Color::White;

    match probe_result {
        WDL::Win => Some(if stm { WDL::Win } else { WDL::Loss }),
        WDL::Draw => Some(WDL::Draw),
        WDL::Loss => Some(if stm { WDL::Loss } else { WDL::Win }),
    }
}
