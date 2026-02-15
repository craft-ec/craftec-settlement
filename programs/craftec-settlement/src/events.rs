use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub admin: Pubkey,
    pub protocol_fee_bps: u16,
}

#[event]
pub struct ConfigUpdated {
    pub protocol_fee_bps: u16,
    pub admin: Pubkey,
    pub paused: bool,
}

#[event]
pub struct CreatorPoolCreated {
    pub creator: Pubkey,
    pub service: u8,
}

#[event]
pub struct PoolDeposited {
    pub creator: Pubkey,
    pub depositor: Pubkey,
    pub amount: u64,
    pub new_balance: u64,
}

#[event]
pub struct RewardsClaimed {
    pub pool: Pubkey,
    pub operator: [u8; 32],
    pub gross_payout: u64,
    pub protocol_fee: u64,
    pub net_payout: u64,
}

#[event]
pub struct ChannelOpened {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub nonce: u64,
    pub amount: u64,
}

#[event]
pub struct ChannelClosed {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub nonce: u64,
    pub receiver_payout: u64,
    pub protocol_fee: u64,
    pub sender_refund: u64,
}

#[event]
pub struct ForceCloseInitiated {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub nonce: u64,
    pub initiated_slot: u64,
    pub claimable_slot: u64,
}

#[event]
pub struct ForceCloseFinalised {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub receiver_payout: u64,
    pub sender_refund: u64,
}
