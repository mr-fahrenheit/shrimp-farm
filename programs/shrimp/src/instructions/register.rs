use crate::{ state::*, error::* };
use crate::helpers::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock;
use solana_program::sysvar;

#[derive(Accounts)]
#[instruction(username: String)]
pub struct Register<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    /// CHECK: Does not impact anything
    #[account()]
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
        space = 8 + UsernameToAddress::INIT_SPACE,
        seeds = [UsernameToAddress::SEED, username.as_bytes(), authority.key().as_ref()],
        bump
    )]
    pub username_to_address_account: Account<'info, UsernameToAddress>,

    #[account(
        init_if_needed,
        payer = player,
        space = 8 + AddressToUsername::INIT_SPACE,
        seeds = [AddressToUsername::SEED, player.key().as_ref(), authority.key().as_ref()],
        bump
    )]
    pub address_to_username_account: Account<'info, AddressToUsername>,

    /// CHECK: account constraints checked in account trait
    #[account(address = sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn register(ctx: Context<Register>, username: String) -> Result<()> {
    // Check transaction restrictions
    limit_instructions(&ctx.accounts.sysvar_instructions, ctx.accounts.game_state.max_ixs, &ctx.accounts.game_state.program_whitelist).unwrap();

    // Username must be 1-12 characters, all lowercase ASCII letters
    if username.len() < 1
        || username.len() > AddressToUsername::MAX_USERNAME_LENGTH
        || !username.chars().all(|c| c.is_ascii_lowercase())
    {
        return err!(CustomErrors::InvalidUsername);
    }

    // Check player has at least a minimum buy
    require!(ctx.accounts.player_state.market_spent + ctx.accounts.player_state.premarket_spent >= MIN_BUY, CustomErrors::MinBuyNotMet);

    // Get state
    let game_state = &mut ctx.accounts.game_state;

    let username_to_address = &mut ctx.accounts.username_to_address_account;
    let address_to_username = &mut ctx.accounts.address_to_username_account;

    // Check if username is taken
    if username_to_address.address != Pubkey::default()
        && username_to_address.address != *ctx.accounts.player.key
    {
        return err!(CustomErrors::UsernameTaken);
    }

    // Check if player address has already registered
    if !address_to_username.username.is_empty() {
        return err!(CustomErrors::AlreadyRegistered);
    }

    // Update state
    address_to_username.username = username.clone();
    username_to_address.address = *ctx.accounts.player.key;
    ctx.accounts.player_state.registered = true;

    // Emit event
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
    emit!(UserRegistered {
        event_index: game_state.event_index,
        player: ctx.accounts.player.key(),
        username,
        timestamp: now,
    });

    // Update indexes
    game_state.event_index = game_state.event_index.checked_add(1).unwrap();

    Ok(())
}

#[event]
pub struct UserRegistered {
    /// The unique sequential index of this event.
    pub event_index: u64,
    /// The address of player registering.
    pub player: Pubkey,
    /// The selected username of player.
    pub username: String,
    /// The timestamp when the event was emitted.
    pub timestamp: u64,
}
