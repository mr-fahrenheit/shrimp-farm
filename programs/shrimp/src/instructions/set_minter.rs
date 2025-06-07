use crate::{state::*};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SetMinter<'info> {
    #[account(mut, address = game_state.authority)]
    authority: Signer<'info>,
    
    #[account(
        seeds = [GameState::SEED, authority.key().as_ref()],
        bump
    )]
    pub game_state: Account<'info, GameState>,
    
    #[account(
        init_if_needed,
        seeds = [MinterState::SEED, authority.key().as_ref()],
        bump,
        payer = authority,
        space = 8 + MinterState::INIT_SPACE
    )]
    pub minter_state: Account<'info, MinterState>,
    
    pub system_program: Program<'info, System>,
}

pub fn set_minter(ctx: Context<SetMinter>, minter: Pubkey) -> Result<()> {
    // Update minter
    ctx.accounts.minter_state.minter = minter;
    
    emit!(MinterSet {
        authority: ctx.accounts.authority.key(),
        minter: minter,
        timestamp: Clock::get()?.unix_timestamp as u64,
    });
    
    Ok(())
}

#[event]
pub struct MinterSet {
    pub authority: Pubkey,
    pub minter: Pubkey,
    pub timestamp: u64,
}