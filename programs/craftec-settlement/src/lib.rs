use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod voucher;

use instructions::*;

declare_id!("fbQtEbQ6dHs9Wpk7xm3vBYkKWgpBkmmK6cpcmn9vvED");

#[program]
pub mod craftec_settlement {
    use super::*;

    pub fn initialize_config(ctx: Context<InitializeConfig>, protocol_fee_bps: u16) -> Result<()> {
        instructions::initialize_config::handler(ctx, protocol_fee_bps)
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        new_fee_bps: Option<u16>,
        new_admin: Option<Pubkey>,
        paused: Option<bool>,
    ) -> Result<()> {
        instructions::update_config::handler(ctx, new_fee_bps, new_admin, paused)
    }

    pub fn create_creator_pool(ctx: Context<CreateCreatorPool>, service: u8) -> Result<()> {
        instructions::create_creator_pool::handler(ctx, service)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::handler(ctx, amount)
    }

    pub fn claim(
        ctx: Context<Claim>,
        operator_pubkey: [u8; 32],
        operator_weight: u64,
        leaf_index: u32,
        merkle_proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::claim::handler(ctx, operator_pubkey, operator_weight, leaf_index, merkle_proof)
    }

    pub fn open_channel(
        ctx: Context<OpenChannel>,
        receiver: Pubkey,
        nonce: u64,
        amount: u64,
    ) -> Result<()> {
        instructions::open_channel::handler(ctx, receiver, nonce, amount)
    }

    pub fn close_channel(
        ctx: Context<CloseChannel>,
        voucher_amount: u64,
        voucher_seq: u64,
        voucher_signature: [u8; 64],
    ) -> Result<()> {
        instructions::close_channel::handler(ctx, voucher_amount, voucher_seq, voucher_signature)
    }

    pub fn force_close(ctx: Context<ForceClose>) -> Result<()> {
        instructions::force_close::handler(ctx)
    }

    pub fn update_voucher(
        ctx: Context<UpdateVoucher>,
        voucher_amount: u64,
        voucher_seq: u64,
        voucher_signature: [u8; 64],
    ) -> Result<()> {
        instructions::update_voucher::handler(ctx, voucher_amount, voucher_seq, voucher_signature)
    }
}
