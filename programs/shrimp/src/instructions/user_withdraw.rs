use crate::{error::*, state::*};
use crate::helpers::*;
use anchor_lang::prelude::*;
use anchor_lang::{
    prelude::{Account, AccountInfo, Program, Result},
    system_program::System,
    ToAccountInfo,
};
use anchor_lang::solana_program::clock;

const INFLATION_FACTOR: u128 = 1 << 64; // Equivalent to 2**64

#[derive(Accounts)]
pub struct UserWithdraw<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    /// CHECK: Custom check
    #[account(address = game_state.authority)]
    pub authority: AccountInfo<'info>,

    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Account<'info, GameState>,

    #[account(
        mut,
        seeds = [player.key().as_ref(), PlayerState::SEED, authority.key().as_ref()],
        bump
    )]
    pub player_state: Account<'info, PlayerState>,

    pub system_program: Program<'info, System>,
}


pub fn user_withdraw(
    ctx: Context<UserWithdraw>,
) -> Result<()> {
    // Get state
    let player_state = &mut ctx.accounts.player_state;
    let game_state = &mut ctx.accounts.game_state;

    // Get current timestamp
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();

    let mut amount: u64 = 0;

    // Calculate referral amount
    let referral_amount = player_state.referral_total.checked_sub(player_state.referral_withdrawn).unwrap();
    amount = amount.checked_add(referral_amount).unwrap();
    player_state.referral_withdrawn = player_state.referral_withdrawn.checked_add(referral_amount).unwrap();

    if now > game_state.premarket_end {
        // Calculate sell amount
        let sell_amount = player_state.sell_total.checked_sub(player_state.sell_withdrawn).unwrap();
        amount = amount.checked_add(sell_amount).unwrap();
        player_state.sell_withdrawn = player_state.sell_withdrawn.checked_add(sell_amount).unwrap();
    }

    // Update and log game state sell and referral balance
    game_state.sell_and_ref_balance = game_state.sell_and_ref_balance.checked_sub(amount).unwrap();

    // Check if player participated in premarket
    if player_state.premarket_spent > 0 && game_state.premarket_balance > 0 && now > game_state.premarket_end {

        let player_premarket_share = (player_state.premarket_spent as u128)
            .checked_mul(INFLATION_FACTOR).unwrap()
            .checked_div(game_state.premarket_spent as u128).unwrap();

        // Calculate share of premarket earnings
        let player_premarket_earned = player_premarket_share
            .checked_mul(game_state.premarket_earned as u128).unwrap()
            .checked_div(INFLATION_FACTOR).unwrap() as u64;

        let premarket_amount_to_withdraw = player_premarket_earned.checked_sub(player_state.premarket_withdrawn).unwrap();

        amount += premarket_amount_to_withdraw;
        player_state.premarket_withdrawn = player_state.premarket_withdrawn.checked_add(premarket_amount_to_withdraw).unwrap();
        game_state.premarket_balance = game_state.premarket_balance.checked_sub(premarket_amount_to_withdraw).unwrap();

        // Check if game is over
        if game_state.game_over && !player_state.prize_withdrawn {
            // Calculate share of final balance
            let player_share_of_final_balance = player_premarket_share
                .checked_mul(game_state.final_balance as u128).unwrap()
                .checked_div(INFLATION_FACTOR).unwrap() as u64;
            amount = amount.checked_add(player_share_of_final_balance).unwrap();
            player_state.prize_withdrawn = true;
        }
    }

    // Require non-zero withdrawal amount
    require!(
        amount > 0,
        self::CustomErrors::InsufficientFunds
    );
    
    // Transfer to user from treasury
    transfer_lamports_from_owned_pda(
        &game_state.to_account_info(),
        &ctx.accounts.player.to_account_info(),
        amount as u64,
    )?;

    // Emit event
    emit!(UserWithdrawn {
        event_index: game_state.event_index,
        amount: amount,
        sell_total: player_state.sell_total,
        player: ctx.accounts.player.key(),
        premarket_withdrawn: player_state.premarket_withdrawn,
        referral_withdrawn: player_state.referral_withdrawn,
        sell_withdrawn: player_state.sell_withdrawn,
        game_over: game_state.game_over,
        prize_withdrawn: player_state.prize_withdrawn,
    });    

    // Update indexes
    game_state.event_index = game_state.event_index.checked_add(1).unwrap();

    Ok(())
}

#[event]
struct UserWithdrawn {
    event_index: u64,
    amount: u64,
    sell_total: u64,
    player: Pubkey,
    premarket_withdrawn: u64,
    referral_withdrawn: u64,
    sell_withdrawn: u64,
    game_over: bool,
    prize_withdrawn: bool,
}
