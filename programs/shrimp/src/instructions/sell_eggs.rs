use crate::account::SellAndHatchAccounts;
use crate::{error::*, state::*};
use crate::helpers::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock;

pub fn sell_eggs(ctx: Context<SellAndHatchAccounts>) -> Result<()> {
    // Check transaction restrictions
    limit_instructions(&ctx.accounts.sysvar_instructions, ctx.accounts.game_state.max_ixs, &ctx.accounts.game_state.program_whitelist).unwrap();

    // Get state
    let player_state = &mut ctx.accounts.player_state;
    let game_state = &mut ctx.accounts.game_state;

    // Get the current time
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();

    // Ensure the game is out of premarket and the sell cooldown has passed
    require!(
        now > game_state.premarket_end,
        CustomErrors::PreMarketInProgress
    );

    // Ensure player sell cooldown is respected
    require!(
        now >= player_state.last_sell + game_state.cooldown,
        CustomErrors::SellCooldownNotReached
    );

    // Ensure the game is not over already
    require!(
        !game_state.game_over,
        CustomErrors::GameOver
    );

    // Calculate total eggs available for sale
    let mut eggs = get_my_eggs(player_state, game_state);

    // Determine bonuses: if the user holds the NFT and if testnet bonus flag is active.
    let mut bonus_percent: u128 = 0;
    if !game_state.collection_key.to_string().eq("11111111111111111111111111111111") {
        if is_nft_holder(
            &ctx.accounts.nft_asset,
            ctx.accounts.player.key(),
            game_state.collection_key
        )? {
            bonus_percent += NFT_BONUS;
        }
    }

    // Add extra 1% bonus for testnet players
    if player_state.testnet_player {
        bonus_percent += TESTNET_BONUS;
    }

    // Add bonus to eggs
    if bonus_percent > 0 {
        eggs = eggs.checked_mul(100 + bonus_percent).unwrap().checked_div(100).unwrap();
    }

    // Ensure that the player has at least 1 full egg unit (as defined by EGGS_TO_HATCH_1SHRIMP)
    require!(eggs >= EGGS_TO_HATCH_1SHRIMP, CustomErrors::NoEggs);

    // Calculate the game balance from treasury by subtracting reserved balances
    let treasury_lamports = **game_state.to_account_info().lamports.borrow();
    let game_balance = treasury_lamports
        .checked_sub(game_state.sell_and_ref_balance).unwrap()
        .checked_sub(game_state.dev_balance).unwrap()
        .checked_sub(game_state.premarket_balance).unwrap();

    // Check if this sell is ending the game
    let new_market_eggs = game_state.market_eggs.checked_add(eggs).unwrap_or(ENDGAME_LIMIT + 1);
    if new_market_eggs > ENDGAME_LIMIT {
        game_state.game_over = true;
        game_state.final_balance = game_balance;
        return Ok(());
    }

    // Calculate the SOL amount for this sale
    let mut egg_sell = calculate_egg_sell(eggs, game_state.market_eggs, game_balance as u128) as u64;

    // Calculate dev and premarket earnings
    let dev_amount = egg_sell.checked_mul(DEV_FEE).unwrap().checked_div(100).unwrap();
    let premarket_amount = egg_sell.checked_mul(PREMARKET_FEE).unwrap().checked_div(100).unwrap();

    // Update the market eggs with the eggs just sold
    game_state.market_eggs = game_state.market_eggs.checked_add(eggs).unwrap();

    // Add to dev balance
    game_state.dev_balance = game_state.dev_balance
        .checked_add(dev_amount)
        .unwrap();

    // Add to premarket earnings
    game_state.premarket_earned = game_state.premarket_earned
        .checked_add(premarket_amount)
        .unwrap();
    game_state.premarket_balance = game_state.premarket_balance
        .checked_add(premarket_amount)
        .unwrap();

    // The user receives remaining 90% of the egg sell value
    egg_sell = egg_sell
        .checked_sub(dev_amount).unwrap()
        .checked_sub(premarket_amount).unwrap();

    // Update the player's sell total and the game's overall sell/referral balance
    player_state.sell_total = player_state.sell_total.checked_add(egg_sell).unwrap();
    game_state.sell_and_ref_balance = game_state.sell_and_ref_balance.checked_add(egg_sell).unwrap();

    // Update the player's timestamps
    player_state.last_interaction = now;
    player_state.last_sell = now;

    // Reset extra eggs (since they have been sold)
    player_state.extra_eggs = 0;

    // Recalculate game balance after the updates
    let treasury_lamports = **game_state.to_account_info().lamports.borrow();
    let new_game_balance = treasury_lamports
        .checked_sub(game_state.sell_and_ref_balance).unwrap()
        .checked_sub(game_state.dev_balance).unwrap()
        .checked_sub(game_state.premarket_balance).unwrap();

    // Emit Sell event
    emit!(Sell {
        game_index: game_state.game_index,
        event_index: game_state.event_index,
        player: ctx.accounts.player.key(),
        market_eggs: game_state.market_eggs,
        game_balance: new_game_balance,
        sol_amount: egg_sell as u64,
        eggs_sold: eggs,
        bonus_percent: bonus_percent as u8,
        timestamp: now,
    });

    // Update indexes
    game_state.event_index = game_state.event_index.checked_add(1).unwrap();
    game_state.game_index  = game_state.game_index.checked_add(1).unwrap();

    Ok(())
}

#[event]
pub struct Sell {
    // The unique sequential index of this game event.
    pub game_index: u64,
    /// The unique sequential index of this event.
    pub event_index: u64,
    /// The player's public key.
    pub player: Pubkey,
    /// The current market eggs value after this event.
    pub market_eggs: u128,
    /// The current game balance (after subtracting reserved amounts).
    pub game_balance: u64,
    /// The SOL amount involved in the event (for Sell this is the SOL received).
    pub sol_amount: u64,
    /// Amount of eggs sold 
    pub eggs_sold: u128,
    // Bonus percent (NFT and testnet player)
    pub bonus_percent: u8,
    /// The timestamp when the event was emitted.
    pub timestamp: u64,
}
