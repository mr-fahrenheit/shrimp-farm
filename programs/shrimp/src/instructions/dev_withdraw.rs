use crate::{error::*, state::*};
use anchor_lang::prelude::*;
use crate::helpers::*;

#[derive(Accounts)]
pub struct DevWithdraw<'info> {
    #[account(mut)]
    signer: Signer<'info>,

    /// CHECK: Can be any
    #[account(address = game_state.authority)]
    authority: UncheckedAccount<'info>,

    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Box<Account<'info, GameState>>,

    /// CHECK: Custom contraint
    #[account(mut, address = game_state.dev1)]
    dev1: UncheckedAccount<'info>,

    /// CHECK: Custom contraint
    #[account(mut, address = game_state.dev2)]
    dev2: UncheckedAccount<'info>,

    /// CHECK: Custom contraint
    #[account(mut, address = game_state.dev3)]
    dev3: UncheckedAccount<'info>,

    system_program: Program<'info, System>,
}

pub fn dev_withdraw(ctx: Context<DevWithdraw>) -> Result<()> {
    // Check one of the devs or authority is a signer
    require!(ctx.accounts.authority.is_signer || ctx.accounts.dev1.is_signer || ctx.accounts.dev2.is_signer || ctx.accounts.dev3.is_signer, self::CustomErrors::InvalidSigner);

    // Retrieve game state data and treasury PDA
    let game_state = &mut ctx.accounts.game_state;
    let treasury_account = &game_state.to_account_info();

    // Calculate rent exemption minimum balance for treasury
    let rent = solana_program::rent::Rent::get().unwrap();
    let rent_exemption = rent.minimum_balance(8 + GameState::INIT_SPACE);

    // Check there are any funds available to withdraw
    require!(game_state.dev_balance > rent_exemption, self::CustomErrors::InsufficientFunds);

    // Calculate amount of dev balance left after rent exemption
    let dev_balance = game_state.dev_balance - rent_exemption;

    // Calculate the base amount (5% of dev_balance)
    let base_amount = dev_balance / 20;

    // Calculate dev shares
    let dev2_amount = base_amount * 8; // 40% of dev_balance
    let dev3_amount = base_amount * 3; // 15% of dev_balance

    // MrF gets the remaining 45%
    let dev1_amount = dev_balance - (dev2_amount + dev3_amount);

    // Reset dev balance
    game_state.dev_balance = rent_exemption;

    // Send 45% from treasury to dev1
    transfer_lamports_from_owned_pda(
        treasury_account,
        &ctx.accounts.dev1,
        dev1_amount as u64,
    )?;

    // Send 40% from treasury to the dev2
    transfer_lamports_from_owned_pda(
        treasury_account,
        &ctx.accounts.dev2,
        dev2_amount as u64,
    )?;

    // Send 15% from treasury to the dev3
    transfer_lamports_from_owned_pda(
        treasury_account,
        &ctx.accounts.dev3,
        dev3_amount as u64,
    )?;

    // Emit event
    emit!(DevWithdrawn {
        dev_balance: dev_balance as u64,
        dev1_amount: dev1_amount as u64,
        dev2_amount: dev2_amount as u64,
        dev3_amount: dev3_amount as u64,
        authority: ctx.accounts.authority.key()
    });

    Ok(())
}

#[event]
struct DevWithdrawn {
    dev_balance: u64,
    dev1_amount: u64,
    dev2_amount: u64,
    dev3_amount:u64,
    authority: Pubkey,
}
