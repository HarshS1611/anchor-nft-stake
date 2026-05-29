use anchor_lang::prelude::*;
use mpl_core::{
    accounts::BaseCollectionV1,
    instructions::CreateV2CpiBuilder,
    types::{DataState, PluginAuthorityPair},
};

use crate::state::StakeConfig;

#[derive(Accounts)]
pub struct MintAsset<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Verified by address constraint — must equal mpl_core::ID
    #[account(mut)]
    pub asset: Signer<'info>,

    #[account(
        mut,
        constraint = collection.key() == config.collection
    )]
    pub collection: Box<Account<'info, BaseCollectionV1>>,

    #[account(
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, StakeConfig>>,

    /// CHECK: Verified by address constraint — must equal mpl_core::ID
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn mint_asset(ctx: Context<MintAsset>, name: String, uri: String) -> Result<()> {
    CreateV2CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .authority(None)
        .payer(&ctx.accounts.payer.to_account_info())
        .owner(Some(&ctx.accounts.payer.to_account_info()))
        .update_authority(None)
        .system_program(&ctx.accounts.system_program.to_account_info())
        .data_state(DataState::AccountState)
        .name(name)
        .uri(uri)
        .plugins(vec![] as Vec<PluginAuthorityPair>)
        .invoke()?;

    Ok(())
}