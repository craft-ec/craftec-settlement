use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct PaymentChannel {
    /// Payer (user)
    pub sender: Pubkey,
    /// Payee (storage/relay node)
    pub receiver: Pubkey,
    /// Nonce for uniqueness
    pub nonce: u64,
    /// Total USDC locked
    pub locked_amount: u64,
    /// Latest cumulative voucher amount
    pub voucher_amount: u64,
    /// Latest voucher sequence number
    pub voucher_seq: u64,
    /// Slot when force_close initiated (0 = not initiated)
    pub force_close_slot: u64,
    /// Channel creation timestamp
    pub created_at: i64,
    /// PDA bump
    pub bump: u8,
}
