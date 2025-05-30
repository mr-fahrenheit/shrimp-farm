use crate::account::BuyAccounts;
use crate::{error::*, state::*};
use crate::helpers::*;
use anchor_lang::prelude::*;
use solana_program::pubkey::Pubkey;
use anchor_lang::solana_program::clock;

pub fn buy_premarket(
    ctx: Context<BuyAccounts>,
    amount: u64, 
) -> Result<()> {
    // Check transaction restrictions
    limit_instructions(&ctx.accounts.sysvar_instructions, ctx.accounts.game_state.max_ixs, &ctx.accounts.game_state.program_whitelist).unwrap();

    // Check amount
    require!(
        amount >= MIN_BUY,
        CustomErrors::BuyAmountTooLow
    );

    // Get state
    let player_state = &mut ctx.accounts.player_state;
    let game_state = &mut ctx.accounts.game_state;
    let referrer = &ctx.accounts.referrer;

    // Check game is in premarket
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
    require!(
        now < game_state.premarket_end,
        CustomErrors::PreMarketOver
    );

    // Transfer SOL from player to treasury
    transfer_lamports(
        &ctx.accounts.player,
        &game_state.to_account_info(),
        &ctx.accounts.system_program,
        amount,
    )?;

    // Update player and game state
    game_state.premarket_spent = game_state.premarket_spent.checked_add(amount).unwrap();
    player_state.premarket_spent = player_state.premarket_spent.checked_add(amount).unwrap();

    // Handle referrals
    let referrer_key = referrer.as_ref().map(|x| x.key());
    if let Some(referrer_pubkey) = referrer_key {
        let referrer_state = &mut ctx.accounts.referrer_state.as_mut().unwrap();
        let (_,_) = process_referral(
            game_state,
            player_state,
            ctx.accounts.player.key(),
            referrer_state,
            referrer_pubkey,
            amount,
        )?;
    }

    // Add dev fee to dev balance
    game_state.dev_balance = game_state
        .dev_balance
        .checked_add(amount.checked_mul(DEV_FEE).unwrap().checked_div(100).unwrap())
        .unwrap();

    // Recalculate the game balance from treasury after subtracting reserved balances
    let treasury_lamports = **game_state.to_account_info().lamports.borrow();
    let game_balance = treasury_lamports
        .checked_sub(game_state.sell_and_ref_balance).unwrap()
        .checked_sub(game_state.dev_balance).unwrap()
        .checked_sub(game_state.premarket_balance).unwrap();

    // Emit event
    emit!(PreMarketBuy {
        game_index:  game_state.game_index,
        event_index: game_state.event_index,
        player: ctx.accounts.player.key(),
        referrer:    player_state.current_referrer,
        game_balance,
        sol_amount: amount,
        timestamp: clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap(),
    });

    // Update indexes
    game_state.event_index = game_state.event_index.checked_add(1).unwrap();
    game_state.game_index  = game_state.game_index.checked_add(1).unwrap();

    Ok(())
}

#[event]
pub struct PreMarketBuy {
    // The unique sequential index of this game event.
    pub game_index: u64,
    /// The unique sequential index of this event.
    pub event_index: u64,
    /// The player's public key.
    pub player: Pubkey,
    /// Will be `Pubkey::default()` (all 1s) if none is set or if the buyer referred themselves.
    pub referrer: Pubkey,
    /// The current game balance (after subtracting reserved amounts).
    pub game_balance: u64,
    /// The SOL amount involved in the event (spent in a buy).
    pub sol_amount: u64,
    /// The timestamp of the event.
    pub timestamp: u64,
}
