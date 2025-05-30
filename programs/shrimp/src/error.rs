use anchor_lang::prelude::*;

#[error_code]
pub enum CustomErrors {
    #[msg("Buy amount below the 0.01 SOL minimum")]
    BuyAmountTooLow,
    #[msg("Amount to withdraw is 0")]
    InsufficientFunds,
    #[msg("PreMarket is in progress")]
    PreMarketInProgress,
    #[msg("No eggs")]
    NoEggs,
    #[msg("Premarket is over")]
    PreMarketOver,
    #[msg("Invalid owner")]
    InvalidOwner,
    #[msg("Invalid collection")]
    InvalidCollection,
    #[msg("Invalid asset")]
    InvalidAsset,
    #[msg("Game Over")]
    GameOver,
    #[msg("Invalid username")]
    InvalidUsername,
    #[msg("Username is taken")]
    UsernameTaken,
    #[msg("Already registered")]
    AlreadyRegistered,
    #[msg("Sell on cooldown")]
    SellCooldownNotReached,
    #[msg("Hatch on cooldown")]
    HatchCooldownNotReached,
    #[msg("Operation allowed only in a test environment")]
    NotTestEnv,
    #[msg("Invalid signer")]
    InvalidSigner,
    #[msg("Initialization locked")]
    InitLocked,
    #[msg("Invalid devs")]
    InvalidDevs,
    #[msg("Collection already set")]
    CollectionAlreadySet,
    #[msg("Bad instruction found")]
    BadInstruction,
    #[msg("Invalid program guards")]
    InvalidProgramGuards,
    #[msg("Must buy before registering")]
    MinBuyNotMet,
    #[msg("Invalid referrer")]
    InvalidReferrer,
}
