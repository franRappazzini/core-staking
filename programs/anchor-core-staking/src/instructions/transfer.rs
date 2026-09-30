use anchor_lang::prelude::*;
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    instructions::TransferV1CpiBuilder,
    types::UpdateAuthority,
    ID as MPL_CORE_ID,
};

use crate::{error::ErrorCode, Oracle};

#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    pub new_owner: SystemAccount<'info>,

    #[account(
        seeds = [b"oracle"],
        bump = oracle.bump,
        constraint = oracle.approve @ ErrorCode::InvalidTimestampToTransfer
    )]
    pub oracle: Account<'info, Oracle>,

    #[account(
        mut,
        has_one = owner @ ErrorCode::InvalidOwner,
        constraint = asset.update_authority == UpdateAuthority::Collection(collection.key()) @ ErrorCode::InvalidUpdateAuthority,
    )]
    pub asset: Account<'info, BaseAssetV1>,

    pub collection: Account<'info, BaseCollectionV1>,

    /// CHECK: This is the ID of the MPL Core Program
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
pub fn handler(ctx: Context<Transfer>) -> Result<()> {
    TransferV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.owner.to_account_info()))
        .new_owner(&ctx.accounts.new_owner.to_account_info())
        .system_program(Some(&ctx.accounts.system_program.to_account_info()))
        .invoke()?;

    Ok(())
}
