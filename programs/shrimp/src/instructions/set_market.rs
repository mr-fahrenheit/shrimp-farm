use crate::{error::*, state::*};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SetMarket<'info> {
    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Account<'info, GameState>,

    #[account(mut, address = game_state.authority)]
    authority: Signer<'info>,
}

pub fn set_market(ctx: Context<SetMarket>, market_eggs: u128) -> Result<()> {
    let game_state = &mut ctx.accounts.game_state;

    // Cannot use on mainnet
    require!(game_state.test_env, CustomErrors::NotTestEnv);

    // Update the market eggs
    game_state.market_eggs = market_eggs;

    emit!(MarketUpdated {
        new_market_eggs: market_eggs,
    });

    Ok(())
}

#[event]
struct MarketUpdated {
    new_market_eggs: u128,
}