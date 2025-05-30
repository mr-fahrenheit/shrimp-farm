use crate::{error::*, state::*};
use anchor_lang::prelude::*;
use solana_program::pubkey::Pubkey;

#[derive(Accounts)]
pub struct SetProgramGuards<'info> {
    #[account(mut, address = game_state.authority)]
    authority: Signer<'info>,

    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Box<Account<'info, GameState>>,
}

pub fn set_program_guards(
    ctx: Context<SetProgramGuards>,
    max_ixs: u8,
    program_whitelist: Vec<Pubkey>,
) -> Result<()> {
    // Validate inputs
    require!(program_whitelist.len() < 10, CustomErrors::InvalidProgramGuards);
    require!(max_ixs < 20, CustomErrors::InvalidProgramGuards);

    // Update state
    let game_state = &mut ctx.accounts.game_state;
    game_state.max_ixs = max_ixs;
    game_state.program_whitelist = program_whitelist;

    Ok(())
}
