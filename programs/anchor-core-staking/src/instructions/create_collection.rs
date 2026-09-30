use anchor_lang::prelude::*;
use mpl_core::{
    instructions::{AddCollectionPluginV1CpiBuilder, CreateCollectionV2CpiBuilder},
    types::{Attribute, Attributes},
    ID as MPL_CORE_ID,
};

#[derive(Accounts)]
pub struct CreateCollection<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub collection: Signer<'info>,
    /// CHECK: This account is not initialized and is being used for signing purposes only, we verify that derives from the correct seeds
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: This is the ID of the MPL Core Program
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
}

/// 3. Collection-Level Staking Stats (Attributes on Collection)
/// Track staking statistics at the collection level using Attributes on the Collection account itself.
///
/// Requirements:
/// - Add a `"total_staked"` counter as an Attribute on the Collection account
/// - Increment on stake, decrement on unstake
pub fn handler(ctx: Context<CreateCollection>, name: String, uri: String) -> Result<()> {
    // Signer seeds for the update authority
    let collection_key = ctx.accounts.collection.key();
    let signer_seeds = &[
        b"update_authority",
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ];

    CreateCollectionV2CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .collection(&ctx.accounts.collection.to_account_info())
        .payer(&ctx.accounts.payer.to_account_info())
        .update_authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .name(name)
        .uri(uri)
        .invoke_signed(&[signer_seeds])?;

    // add the total_staked attribute to collection
    let attribute_list = vec![Attribute {
        key: "total_staked".to_string(),
        value: "0".to_string(),
    }];

    AddCollectionPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .collection(&ctx.accounts.collection.to_account_info())
        .payer(&ctx.accounts.payer.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(mpl_core::types::Plugin::Attributes(Attributes {
            attribute_list,
        }))
        .invoke_signed(&[signer_seeds])?;

    Ok(())
}
