use anchor_lang::prelude::*;
use crate::state::*;

#[derive(Accounts)]
pub struct TestnetBonus<'info> {
    #[account(mut)]
    authority: Signer<'info>,

    /// CHECK: Can be any player
    #[account()]
    pub player: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + PlayerState::INIT_SPACE,
        seeds = [player.key().as_ref(), PlayerState::SEED, authority.key().as_ref()],
        bump
    )]
    pub player_state: Account<'info, PlayerState>,

    pub system_program: Program<'info, System>,
}

/// This function toggles the `testnet_player` flag on the player's state.
pub fn testnet_bonus(ctx: Context<TestnetBonus>) -> Result<()> {
    let player_state = &mut ctx.accounts.player_state;

    // Flip the bonus flag (i.e. true becomes false and vice versa)
    player_state.testnet_player = !player_state.testnet_player;
    
    // Emit event
    emit!(TestnetBonusEvent {
        player: ctx.accounts.player.key(),
        new_state: player_state.testnet_player,
    });

    Ok(())
}

#[event]
pub struct TestnetBonusEvent {
    pub player: Pubkey,
    pub new_state: bool,
}