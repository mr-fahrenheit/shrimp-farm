use crate::{error::*, state::*};
use anchor_lang::prelude::*;
use solana_program::pubkey::Pubkey;
use anchor_lang::solana_program::clock;

#[derive(Accounts)]
pub struct EndPremarket<'info> {
    #[account(mut, address = game_state.authority)]
    authority: Signer<'info>,

    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Box<Account<'info, GameState>>,
}

pub fn end_premarket(
    ctx: Context<EndPremarket>,
) -> Result<()> {
    // Get game state account
    let game_state = &mut ctx.accounts.game_state;

    // Cannot use on mainnet
    require!(game_state.test_env, CustomErrors::NotTestEnv);

    // Check game is actually in premarket
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
    require!(
        now < game_state.premarket_end,
        CustomErrors::PreMarketOver
    );

    // End premarket immediately
    game_state.premarket_end = now - 1;

    Ok(())
}
