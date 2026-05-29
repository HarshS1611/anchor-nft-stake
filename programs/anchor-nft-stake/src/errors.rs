use anchor_lang::prelude::*;

#[error_code]
pub enum StakingError {
    #[msg("The freeze period has not passed yet")]
    FreezePeriodNotPassed,

    #[msg("No rewards available to claim")]
    NothingToClaim,

    #[msg("NFT does not belong to the staking collection")]
    InvalidCollection,

    #[msg("This NFT is already staked")]
    AlreadyStaked,

    #[msg("This NFT is not staked")]
    NotStaked,

    #[msg("Overflow when calculating rewards")]
    RewardOverflow,
}