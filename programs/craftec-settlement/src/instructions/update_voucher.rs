use anchor_lang::prelude::*;

use crate::state::PaymentChannel;
use crate::errors::SettlementError;
use crate::voucher::verify_voucher;

pub fn handler(
    ctx: Context<UpdateVoucher>,
    voucher_amount: u64,
    voucher_seq: u64,
    voucher_signature: [u8; 64],
) -> Result<()> {
    let channel_key = ctx.accounts.channel.key();
    let channel = &mut ctx.accounts.channel;

    require!(
        verify_voucher(
            &channel.sender.to_bytes(),
            &channel_key,
            voucher_amount,
            voucher_seq,
            &voucher_signature,
        ),
        SettlementError::InvalidVoucherSignature,
    );

    require!(voucher_seq > channel.voucher_seq, SettlementError::StaleVoucher);
    require!(voucher_amount <= channel.locked_amount, SettlementError::VoucherExceedsLocked);

    channel.voucher_amount = voucher_amount;
    channel.voucher_seq = voucher_seq;

    // Reset force_close if initiated (receiver proved liveness)
    if channel.force_close_slot > 0 {
        channel.force_close_slot = 0;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateVoucher<'info> {
    pub receiver: Signer<'info>,

    #[account(
        mut,
        seeds = [b"payment_channel", channel.sender.as_ref(), channel.receiver.as_ref(), &channel.nonce.to_le_bytes()],
        bump = channel.bump,
        constraint = receiver.key() == channel.receiver @ SettlementError::NotAuthorized,
    )]
    pub channel: Account<'info, PaymentChannel>,
}
