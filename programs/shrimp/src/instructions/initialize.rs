use crate::{error::*, state::*};
use anchor_lang::prelude::*;
use anchor_lang::{
    prelude::{Account, Program, Result},
    system_program::System,
    ToAccountInfo,
};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    owner: Signer<'info>,

    #[account(mut)]
    authority: Signer<'info>,

    #[account(
        init_if_needed,
        seeds = [LockState::SEED], 
        bump, 
        payer = authority, 
        space = 8 + LockState::INIT_SPACE
    )]
    pub lock_state: Account<'info, LockState>,

    #[account(
        init, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump, 
        payer = authority, 
        space = 8 + GameState::INIT_SPACE
    )]
    pub game_state: Account<'info, GameState>,

    system_program: Program<'info, System>,
}

pub fn initialize(
    ctx: Context<Initialize>,
    dev1: Pubkey,
    dev2: Pubkey,
    dev3: Pubkey,
    premarket_end: u64,
    cooldown: u64,
    test_env: bool,
) -> Result<()> {
    // Check dev keys
    require!(dev1 != dev2 && dev1 != dev3 && dev2 != dev3, CustomErrors::InvalidDevs);

    // Check owner
    require!(ctx.accounts.owner.key() == pubkey!("CdKqXMm7QDjMwfFR3GgWTRQE7x39BFbiLm8KWC4TibzR"), CustomErrors::InvalidOwner);

    // Initialize game state
    let game_state = &mut ctx.accounts.game_state;
    game_state.authority = ctx.accounts.authority.key();
    game_state.market_eggs = MARKET_START.into();
    game_state.dev1 = dev1;
    game_state.dev2 = dev2;
    game_state.dev3 = dev3;
    game_state.dev_balance = **game_state.to_account_info().lamports.borrow();
    game_state.event_index = 1;
    game_state.game_index = 1;
    game_state.premarket_end = premarket_end;
    game_state.cooldown = cooldown;
    game_state.test_env = test_env;
    game_state.max_ixs = 5;
    game_state.program_whitelist = vec!();

    // For mainnet deploy, prevent multiple initializes
    require!(!ctx.accounts.lock_state.locked, CustomErrors::InitLocked);
    if !test_env {
        ctx.accounts.lock_state.locked = true;
    }

    // Emit event
    emit!(Initialized {
        dev1: dev1,
        dev2: dev2,
        dev3: dev3,
        owner: ctx.accounts.authority.key(),
        premarket_end: premarket_end,
        test_env: test_env
    }); 
    
    // Success
    Ok(())
}

#[event]
struct Initialized {
    dev1: Pubkey,
    dev2: Pubkey,
    dev3: Pubkey,
    owner: Pubkey,
    premarket_end: u64,
    test_env: bool
}
