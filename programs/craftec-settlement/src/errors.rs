use anchor_lang::prelude::*;

#[error_code]
pub enum SettlementError {
    #[msg("Protocol fee exceeds maximum (50%)")]
    FeeTooHigh,
    #[msg("Program is paused")]
    Paused,
    #[msg("Not authorized")]
    NotAuthorized,
    #[msg("Invalid service type")]
    InvalidService,
    #[msg("Amount must be > 0")]
    InvalidAmount,
    #[msg("Distribution already posted")]
    DistributionAlreadyPosted,
    #[msg("Distribution not yet posted")]
    DistributionNotPosted,
    #[msg("Insufficient pool balance")]
    InsufficientPoolBalance,
    #[msg("Invalid Merkle proof")]
    InvalidMerkleProof,
    #[msg("No receipts in distribution")]
    NoReceipts,
    #[msg("Voucher amount exceeds locked")]
    VoucherExceedsLocked,
    #[msg("Stale voucher (seq too low)")]
    StaleVoucher,
    #[msg("Invalid voucher signature")]
    InvalidVoucherSignature,
    #[msg("Force close timeout not elapsed")]
    ForceCloseTimeout,
    #[msg("Force close already initiated")]
    ForceCloseAlreadyInitiated,
}
