use anchor_lang::prelude::*;

// Game Constants

pub const PSN: u128 = 10000;
pub const PSNH: u128 = 5000;
pub const ENDGAME_LIMIT: u128 = (10 as u128).pow(34);

pub const EGGS_TO_HATCH_1SHRIMP: u128 = 86400;  // Assuming 1 day's worth of seconds

pub const MARKET_START: u128 = 864000000000;    // 10 million eggs
pub const NFT_MIN_BUY: u64 = 1000000000;        // 1 SOL minimum buy to get NFT
pub const MIN_BUY: u64 = 10000000;              // 0.01 SOL minimum buy

// Fee and bonus constants

pub const DEV_FEE: u64 = 4;                     // 4% to devs
pub const PREMARKET_FEE: u64 = 6;               // 6% to premarket
pub const FEE: u64 = DEV_FEE + PREMARKET_FEE;   // Total fee (dev + premarket)
pub const REFERRAL_FEE: u64 = 4;                // 4% bonus to referrer
pub const REFERRAL_CASHBACK: u64 = 1;           // 1% cashback to referree
pub const NFT_BONUS: u128 = 10;                 // 10% bonus for NFT holders
pub const TESTNET_BONUS: u128 = 1;              // 1% bonus for testnet players

// Seed constants

pub const CANDY_MACHINE_AUTHORITY_SEED: &str = "candy_machine";

// ───────────────────────── Player State ──────────────────────────
#[account]
#[derive(InitSpace)]
pub struct PlayerState {
    // Production
    pub shrimp: u128,            // Current shrimp owned by the player
    pub extra_eggs: u128,        // Eggs generated at old (lower) rate

    // Activity timestamps (Unix seconds)
    pub last_interaction: u64,   // Last action of any kind (hatch or sell)
    pub last_hatch: u64,         // Last time the player hatched eggs
    pub last_sell: u64,          // Last time the player sold eggs

    // Earnings (Lamports)
    pub referral_total: u64,     // Lifetime referral or cashback income
    pub sell_total: u64,         // Lifetime income from selling eggs
    pub referral_withdrawn: u64, // Lamports withdrawn from referral earnings
    pub sell_withdrawn: u64,     // Lamports withdrawn from sell earnings
    pub premarket_withdrawn: u64,// Lamports withdrawn from pre-market earnings

    // Expenditure (lamports)
    pub premarket_spent: u64,    // Lamports spent during the pre-market phase
    pub market_spent: u64,       // Lamports spent in the primary market

    // Relationships
    pub current_referrer: Pubkey,// Address used as referrer for new purchases

    // Status flags
    pub prize_withdrawn: bool,   // Whether the final-prize payout has been claimed
    pub minted: bool,            // True if the player’s NFT is already minted
    pub testnet_player: bool,    // Grants +1 % production if the player joined testnet
    pub registered: bool,        // True if player has registered a username
}

impl PlayerState {
    pub const SEED: &'static [u8] = b"shrimp";
}

// ────────────────────────── Game State ───────────────────────────
#[account]
#[derive(InitSpace)]
pub struct GameState {
    // Authority and dev accounts
    pub authority: Pubkey,       // Master authority PDA seed
    pub dev1: Pubkey,            // Developer 1
    pub dev2: Pubkey,            // Developer 2
    pub dev3: Pubkey,            // Developer 3

    // NFT infrastructure
    pub collection_key: Pubkey,  // NFT collection address
    pub candymachine_key: Pubkey,// Candy Machine that mints the NFTs

    // Economic parameters
    pub cooldown: u64,           // Minimum delay (sec) between hatch / sell
    pub market_eggs: u128,       // Eggs circulating in the open market

    // Premarket window
    pub premarket_end: u64,      // Timestamp when pre-market closes
    pub premarket_spent: u64,    // Global spending during the pre-market
    pub premarket_balance: u64,  // Current treasury of pre-market earnings
    pub premarket_earned: u64,   // Lifetime pre-market earnings

    // Balances (lamports)
    pub sell_and_ref_balance: u64,// Funds reserved for user sells & referrals
    pub dev_balance: u64,        // Accumulated dev fee balance
    pub final_balance: u64,      // Treasury for the end-game prize

    // Game progression
    pub game_over: bool,         // Set to true once the game is concluded
    pub event_index: u64,        // Incrementing event counter (global)
    pub game_index: u64,         // Unique identifier for the current game action
    pub nfts_minted: u16,        // Total NFTs minted so far

    // Environment options
    pub test_env: bool,          // Enables on-chain test functions

    // Program whitelist
    pub max_ixs: u8,
    #[max_len(10)]
    pub program_whitelist: Vec<Pubkey>,
}

impl GameState {
    pub const SEED: &'static [u8] = b"shrimp";
}

#[account]
#[derive(InitSpace)]
pub struct LockState {
    pub locked: bool,
}

impl LockState {
    pub const SEED: &'static [u8] = b"shrimplock";
}

// Usernames
#[account]
#[derive(InitSpace)]
pub struct UsernameToAddress {
    pub address: Pubkey,
}

impl UsernameToAddress {
    pub const SEED: &'static [u8] = b"username_to_address";
}

#[account]
#[derive(InitSpace)]
pub struct AddressToUsername {
    #[max_len(12)]
    pub username: String,
}

impl AddressToUsername {
    pub const MAX_USERNAME_LENGTH: usize = 12;
    pub const SEED: &'static [u8] = b"address_to_username";
}