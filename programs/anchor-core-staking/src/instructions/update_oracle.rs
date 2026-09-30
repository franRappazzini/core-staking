use crate::Oracle;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateOracle<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"oracle"],
        bump = oracle.bump
    )]
    pub oracle: Account<'info, Oracle>,

    // CHECK: just pda to hold rewards lamports
    #[account(
        mut,
        seeds = [b"oracle_vault"],
        bump = oracle.oracle_vault_bump
    )]
    pub oracle_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<UpdateOracle>) -> Result<()> {
    ctx.accounts.oracle.update()?;

    // if the update was successfully, we transfer incentives from incentive vault to signer
    ctx.accounts
        .oracle_vault
        .sub_lamports(ctx.accounts.oracle.incentives)?;
    ctx.accounts
        .signer
        .add_lamports(ctx.accounts.oracle.incentives)?;

    Ok(())
}
