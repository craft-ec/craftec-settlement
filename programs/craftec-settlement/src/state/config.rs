use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    /// Admin authority
    pub admin: Pubkey,
    /// Protocol fee in basis points (e.g. 500 = 5%)
    pub protocol_fee_bps: u16,
    /// Treasury token account pubkey (for validation)
    pub treasury: Pubkey,
    /// Emergency pause flag
    pub paused: bool,
    /// PDA bump
    pub bump: u8,
}
