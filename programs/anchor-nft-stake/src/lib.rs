use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;
 
use instructions::*;

declare_id!("9fGZzouDA77HFRAWTd1Kq1WV7BPW891D4eGUvUatPvq2");

#[program]
pub mod anchor_nft_stake {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize::initialize(ctx)
    }
 
    pub fn mint_asset(ctx: Context<MintAsset>, name: String, uri: String) -> Result<()> {
        instructions::mint_asset::mint_asset(ctx, name, uri)
    }
 
    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        instructions::stake::stake(ctx)
    }
 
    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        instructions::unstake::unstake(ctx)
    }
 
    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        instructions::claim_rewards::claim_rewards(ctx)
    }
}
