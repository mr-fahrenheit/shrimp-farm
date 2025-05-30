use crate::account::SellAndHatchAccounts;
use crate::{error::*, state::*};
use crate::helpers::*;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock;

pub fn hatch_eggs(ctx: Context<SellAndHatchAccounts>) -> Result<()> {
    // Check transaction restrictions
    limit_instructions(&ctx.accounts.sysvar_instructions, ctx.accounts.game_state.max_ixs, &ctx.accounts.game_state.program_whitelist).unwrap();

    // Get state
    let player_state = &mut ctx.accounts.player_state;
    let game_state = &mut ctx.accounts.game_state;

    // Get the current time
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();

    // Ensure the game is out of premarket, hatch cooldown passed, game not over
    require!(
        now > game_state.premarket_end,
        CustomErrors::PreMarketInProgress
    );

    // Ensure player hatch cooldown is respected
    require!(
        now >= player_state.last_hatch + game_state.cooldown,
        CustomErrors::HatchCooldownNotReached
    );

    // Ensure the game is not over already
    require!(
        !game_state.game_over,
        CustomErrors::GameOver
    );

    // Calculate total eggs available for hatching
    let mut eggs = get_my_eggs(player_state, game_state);

    // Determine bonuses: the user holds the NFT and if testnet bonus flag is active.
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

    // Convert eggs to shrimp using the conversion ratio
    let shrimp_to_add = eggs.checked_div(EGGS_TO_HATCH_1SHRIMP).unwrap_or_default();

    // Must have at least 1 egg
    require!(shrimp_to_add >= 1, CustomErrors::NoEggs);

    // Update the player's shrimp count
    player_state.shrimp = player_state.shrimp.checked_add(shrimp_to_add).unwrap_or(player_state.shrimp);

    // Reset extra eggs (since they have been hatched)
    player_state.extra_eggs = 0;

    // Update timestamps
    player_state.last_interaction = now;
    player_state.last_hatch = now;

    // Emit Hatch event
    emit!(Hatch {
        game_index:  game_state.game_index,
        event_index: game_state.event_index,
        player:      ctx.accounts.player.key(),
        shrimp:      shrimp_to_add,
        bonus_percent: bonus_percent as u8,
        timestamp:   now,
    });

    // Update indexes
    game_state.event_index = game_state.event_index.checked_add(1).unwrap();
    game_state.game_index  = game_state.game_index.checked_add(1).unwrap();

    Ok(())
}

#[event]
pub struct Hatch {
    // The unique sequential index of this game event.
    pub game_index: u64,
    /// The unique sequential index of this event.
    pub event_index: u64,
    /// The player's public key.
    pub player: Pubkey,
    /// For Hatch events: the amount of shrimp added.
    pub shrimp: u128,
    // Bonus percent (NFT and testnet player)
    pub bonus_percent: u8,
    /// The timestamp when the event was emitted.
    pub timestamp: u64,
}
