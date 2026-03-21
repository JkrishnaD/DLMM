use anchor_lang::prelude::*;

#[error_code]
pub enum DlmmErrors {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Invalid bin range")]
    InvalidBinRange,
    #[msg("Bin range exceeds maximum of 70 bins per position")]
    RangeExceedMaxBins,
    #[msg("Zero amount")]
    ZeroAmount,
    #[msg("Invalid token amount for bin range")]
    InvalidTokenAmount,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Bin array not found")]
    BinArrayNotFound,
    #[msg("Zero shares")]
    ZeroShares,
    #[msg("Invalid bin array")]
    InvalidBinArray,
    #[msg("Out of bounds")]
    OutOfBounds,
    #[msg("Invalid bin index")]
    InvalidBinIndex,
}
