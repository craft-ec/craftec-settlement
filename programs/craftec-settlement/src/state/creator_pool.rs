use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct CreatorPool {
    /// Creator's public key
    pub creator: Pubkey,
    /// Service type (Data=1, Bundle=4)
    pub service: u8,
    /// Current USDC balance
    pub pool_balance: u64,
    /// Account creation timestamp
    pub created_at: i64,
    /// Epoch counter
    pub current_epoch: u64,
    /// Balance snapshot at distribution posting
    pub epoch_balance: u64,
    /// Total receipt weight for current distribution
    pub total_weight: u64,
    /// Merkle root of current distribution
    pub distribution_root: [u8; 32],
    /// Whether a distribution is active
    pub distribution_posted: bool,
    /// Timestamp of last distribution posting
    pub last_distribution_at: i64,
    /// PDA bump
    pub bump: u8,
}
