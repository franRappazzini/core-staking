use crate::Oracle;
use anchor_lang::{prelude::*, solana_program::native_token::LAMPORTS_PER_SOL, system_program};

#[derive(Accounts)]
pub struct InitOracle<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = Oracle::LEN,
        seeds = [b"oracle"],
        bump
    )]
    pub oracle: Account<'info, Oracle>,

    /// CHECK: just pda to hold rewards lamports
    #[account(
        init,
        payer = signer,
        space = 0,
        seeds = [b"oracle_vault"],
        bump
    )]
    pub oracle_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitOracle>, incentives: u64) -> Result<()> {
    // set oracle account
    ctx.accounts.oracle.incentives = incentives;
    ctx.accounts.oracle.oracle_vault_bump = ctx.bumps.oracle_vault;
    ctx.accounts.oracle.bump = ctx.bumps.oracle;

    // we update the oracle to set the first valid state
    ctx.accounts.oracle.update()?;

    // we transfer 1 sol to vault to then pay rewards
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.signer.to_account_info(),
                to: ctx.accounts.oracle_vault.to_account_info(),
            },
        ),
        LAMPORTS_PER_SOL,
    )
}
