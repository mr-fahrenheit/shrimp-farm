use crate::account::BuyAccounts;
use crate::{error::*, state::*};
use crate::helpers::*;
use anchor_lang::prelude::*;
use solana_program::pubkey::Pubkey;
use anchor_lang::solana_program::clock;

pub fn buy_shrimp(
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

    let player_state = &mut ctx.accounts.player_state;
    let game_state = &mut ctx.accounts.game_state;
    let referrer = &ctx.accounts.referrer;

    // Check game is out of premarket
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
    require!(
        now > game_state.premarket_end,
        CustomErrors::PreMarketInProgress
    );

    // Ensure the game is not over already
    require!(
        !game_state.game_over,
        CustomErrors::GameOver
    );

    // Calculate game balance from treasury
    let treasury_lamports = **game_state.to_account_info().lamports.borrow();
    let game_balance = treasury_lamports
        .checked_sub(game_state.sell_and_ref_balance).unwrap()        
        .checked_sub(game_state.dev_balance).unwrap()
        .checked_sub(game_state.premarket_balance).unwrap();
        
    // Calculate eggs bought
    let eggs_bought: u128 = calculate_egg_buy(amount as u128, game_balance as u128, game_state.market_eggs);

    // Calculate current eggs and add them to extra_eggs if present
    let new_eggs = get_eggs_since_last_hatch(player_state, game_state);
    if new_eggs > 0 {
        player_state.extra_eggs = player_state.extra_eggs.checked_add(new_eggs).unwrap_or(player_state.extra_eggs);
    }

    // Transfer SOL to the treasury
    transfer_lamports(
        &ctx.accounts.payer,
        &game_state.to_account_info(),
        &ctx.accounts.system_program,
        amount
    )?;

    // Add dev fee to dev balance
    game_state.dev_balance = game_state.dev_balance
        .checked_add(amount.checked_mul(DEV_FEE).unwrap().checked_div(100).unwrap())
        .unwrap();

    // Add to premarket balance
    let premarket_fee = amount.checked_mul(PREMARKET_FEE).unwrap().checked_div(100).unwrap();
    game_state.premarket_earned = game_state.premarket_earned
        .checked_add(premarket_fee)
        .unwrap();
    game_state.premarket_balance = game_state.premarket_balance
        .checked_add(premarket_fee)
        .unwrap();

    // Convert eggs to shrimp
    let shrimp_to_add = eggs_bought.checked_div(EGGS_TO_HATCH_1SHRIMP).unwrap_or_default();
    player_state.shrimp = player_state.shrimp.checked_add(shrimp_to_add).unwrap_or(player_state.shrimp);

    // Handle referrals
    let referrer_key = referrer.as_ref().map(|x| x.key());
    if let Some(referrer_pubkey) = referrer_key {
        let referrer_state = &mut ctx.accounts.referrer_state.as_mut().unwrap();
        let (_, _) = process_referral(
            game_state,
            player_state,
            ctx.accounts.player.key(),
            referrer_state,
            referrer_pubkey,
            amount,
            ctx.accounts.payer.key(),
        )?;
    }

    // Update last interaction and market spend
    player_state.last_interaction = now;
    player_state.market_spent = player_state.market_spent.checked_add(amount).unwrap();

    // Recalculate the game balance after state updates
    let treasury_lamports = **game_state.to_account_info().lamports.borrow();
    let game_balance = treasury_lamports
        .checked_sub(game_state.sell_and_ref_balance).unwrap()
        .checked_sub(game_state.dev_balance).unwrap()
        .checked_sub(game_state.premarket_balance).unwrap();

    // Emit an event for Buy
    emit!(Buy {
        game_index:  game_state.game_index,
        event_index: game_state.event_index,
        player: ctx.accounts.player.key(),
        referrer:    player_state.current_referrer,
        game_balance,
        sol_amount: amount,
        shrimp: shrimp_to_add,
        extra_eggs: player_state.extra_eggs,
        timestamp: player_state.last_interaction,
    });

    // Update indexes
    game_state.event_index = game_state.event_index.checked_add(1).unwrap();
    game_state.game_index  = game_state.game_index.checked_add(1).unwrap();

    Ok(())
}

#[event]
pub struct Buy {
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
    /// For buy and hatch events: the amount of shrimp added.
    pub shrimp: u128,
    /// Only for buy, extra eggs
    pub extra_eggs: u128,
    /// The timestamp of the event.
    pub timestamp: u64,
}
