use crate::state::*;
use anchor_lang::prelude::*;
use mpl_core::accounts::BaseAssetV1;
use crate::state::GameState;
use solana_program::{pubkey::Pubkey, sysvar};

#[derive(Accounts)]
pub struct BuyAccounts<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    /// CHECK: Custom check
    #[account(address = game_state.authority)]
    pub authority: AccountInfo<'info>,

    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Box<Account<'info, GameState>>,

    #[account(
        init_if_needed,
        payer = player,
        space = 8 + PlayerState::INIT_SPACE,
        seeds = [player.key().as_ref(), PlayerState::SEED, authority.key().as_ref()],
        bump
    )]
    pub player_state: Box<Account<'info, PlayerState>>,

    #[account(
        init_if_needed,
        payer = player,
        space = 8 + PlayerState::INIT_SPACE,
        seeds = [referrer.as_ref().unwrap().key().as_ref(), PlayerState::SEED, authority.key().as_ref()],
        bump
    )]
    pub referrer_state: Option<Box<Account<'info, PlayerState>>>,

    /// CHECK: Can be any account
    #[account()]
    pub referrer: Option<AccountInfo<'info>>,

    /// Instructions sysvar account.
    ///
    /// CHECK: account constraints checked in account trait
    #[account(address = sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SellAndHatchAccounts<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    /// CHECK: Custom check
    #[account(address = game_state.authority)]
    pub authority: AccountInfo<'info>,

    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Account<'info, GameState>,

    #[account(mut,
        seeds = [player.key().as_ref(), PlayerState::SEED, authority.key().as_ref()],
        bump
    )]
    pub player_state: Account<'info, PlayerState>,

    #[account(mut)]
    pub nft_asset: Option<Account<'info, BaseAssetV1>>,

    /// Instructions sysvar account.
    ///
    /// CHECK: account constraints checked in account trait
    #[account(address = sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}