use cozy_chess::{Board, Color, Piece, Square};

use crate::batch::EntryFeatureWriter;

use super::InputFeatureSet;

pub struct Board768;

impl InputFeatureSet for Board768 {
    const MAX_INDICES: usize = 64;

    fn add_features(board: Board, entry: EntryFeatureWriter) {
        let mut sparse_entry = entry.sparse();
        let stm = board.side_to_move();

        for &color in &Color::ALL {
            for &piece in &Piece::ALL {
                for square in board.pieces(piece) & board.colors(color) {
                    let stm_feature = feature(stm, color, piece, square);
                    let nstm_feature = feature(!stm, color, piece, square);
                    sparse_entry.add_feature(stm_feature as i64, nstm_feature as i64);
                }
            }
        }
    }
}

fn feature(perspective: Color, color: Color, piece: Piece, square: Square) -> usize {
    let (square, color) = match perspective {
        Color::White => (square, color),
        Color::Black => (square.flip_rank(), !color),
    };
    let mut index = 0;
    index = index * Color::NUM + color as usize;
    index = index * Piece::NUM + piece as usize;
    index = index * Square::NUM + square as usize;
    index
}
