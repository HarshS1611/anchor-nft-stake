use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenInterface};
use mpl_core::accounts::BaseCollectionV1;

use crate::state::{StakeConfig, UserAccount};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    pub collection: Box<Account<'info, BaseCollectionV1>>,

    #[account(
        init,
        payer = admin,
        mint::decimals = 6,
        mint::authority = config,
        mint::token_program = token_program,
    )]
    pub rewards_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = admin,
        space = 8 + StakeConfig::INIT_SPACE,
        seeds = [b"config"],
        bump
    )]
    pub config: Box<Account<'info, StakeConfig>>,

    #[account(
        init,
        payer = admin,
        space = 8 + UserAccount::INIT_SPACE,
        seeds = [b"user", admin.key().as_ref()],
        bump
    )]
    pub user_account: Box<Account<'info, UserAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.rewards_per_day = 10;
    config.freeze_period = 0;
    config.rewards_mint = ctx.accounts.rewards_mint.key();
    config.collection = ctx.accounts.collection.key();
    config.bump = ctx.bumps.config;

    let user = &mut ctx.accounts.user_account;
    user.points = 0;
    user.amount_staked = 0;
    user.bump = ctx.bumps.user_account;

    Ok(())
}