use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo};

declare_id!("9rBKpkU7gkq7nndgQuhhped2zQdt5pYwfAUH2XpsfBch");

/// Constants for risk management and flash loan protection.
const MIN_STAKE_DURATION: i64 = 60; // Minimum staking duration in seconds.
const MAX_ALLOWED_LOSS: u64 = 50;   // Maximum allowed loss percentage before liquidation.

#[program]
pub mod dnt_perpetual_token {
    use super::*;

    // Initialize the global protocol state.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.bump = ctx.bumps.state;
        state.total_staked = 0;
        let now = Clock::get()?.unix_timestamp;
        state.last_update = now;
        state.last_rebalance = now;
        // Default governance risk parameter.
        state.allowed_delta_threshold = 100;
        Ok(())
    }

    // Stake tokens to join the automated trading pool.
    pub fn stake(ctx: Context<StakeAccounts>, amount: u64) -> Result<()> {
        // Transfer tokens from the trader’s account to the vault.
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.vault_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        token::transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts),
            amount,
        )?;

        let state = &mut ctx.accounts.state;
        state.total_staked = state.total_staked.checked_add(amount).unwrap();

        let user_stake = &mut ctx.accounts.user_stake;
        user_stake.amount = user_stake.amount.checked_add(amount).unwrap();
        user_stake.last_update = Clock::get()?.unix_timestamp;
        Ok(())
    }

    // Stake using multiple collateral types (e.g., SOL, USDC, USDT).
    pub fn stake_with_multiple_assets(
        ctx: Context<MultiCollateralStakeAccounts>,
        asset_type: u8,
        amount: u64,
    ) -> Result<()> {
        // Convert the provided amount to a normalized value.
        let conversion_rate = get_conversion_rate(asset_type)?;
        let normalized_amount = amount.checked_mul(conversion_rate).unwrap();

        let state = &mut ctx.accounts.state;
        state.total_staked = state.total_staked.checked_add(normalized_amount).unwrap();

        let user_stake = &mut ctx.accounts.user_stake;
        user_stake.amount = user_stake.amount.checked_add(normalized_amount).unwrap();
        user_stake.last_update = Clock::get()?.unix_timestamp;

        // Transfer the provided tokens from the user to the vault.
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.vault_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        token::transfer(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts),
            amount,
        )?;
        Ok(())
    }

    // Unstake tokens and withdraw from the pool.
    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        let user_stake = &mut ctx.accounts.user_stake;
        require!(user_stake.amount >= amount, CustomError::InsufficientStake);

        // Enforce a minimum staking duration to help prevent flash loan exploits.
        let now = Clock::get()?.unix_timestamp;
        require!(
            now.checked_sub(user_stake.last_update).unwrap() >= MIN_STAKE_DURATION,
            CustomError::EarlyUnstakeNotAllowed
        );

        user_stake.amount = user_stake.amount.checked_sub(amount).unwrap();
        let state = &mut ctx.accounts.state;
        state.total_staked = state.total_staked.checked_sub(amount).unwrap();

        // Prepare PDA seeds for signing.
        let seeds = &[b"state", ctx.accounts.state_owner.key.as_ref(), &[state.bump]];
        let signer = &[&seeds[..]];
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.state.to_account_info(),
        };
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                signer,
            ),
            amount,
        )?;
        Ok(())
    }

    // Rebalance positions to maintain delta-neutral exposure.
    pub fn rebalance(ctx: Context<Rebalance>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.last_rebalance = Clock::get()?.unix_timestamp;
        Ok(())
    }

    // Distribute rewards to staked participants.
    // This simplified calculation multiplies the total stake by a reward rate and the staking duration.
    pub fn distribute_rewards(ctx: Context<DistributeRewards>) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let duration = current_time.checked_sub(ctx.accounts.state.last_update).unwrap() as u64;
        let reward_rate: u64 = 1; // Placeholder reward rate.
        let reward_amount = ctx.accounts.state
            .total_staked
            .checked_mul(reward_rate)
            .unwrap()
            .checked_mul(duration)
            .unwrap();

        mint_rewards(
            &ctx.accounts.state,
            &ctx.accounts.state_owner,
            &ctx.accounts.token_mint,
            &ctx.accounts.rewards_account,
            &ctx.accounts.token_program,
            reward_amount,
        )?;
        ctx.accounts.state.last_update = current_time;
        Ok(())
    }

    // 1️⃣ Dynamic Funding Rate Distribution.
    // Adjust rewards based on real-time funding rates from the perpetual futures market.
    pub fn update_rewards_based_on_funding(ctx: Context<UpdateRewards>) -> Result<()> {
        let funding_rate = get_funding_rate_from_oracle()?;
        let reward_amount = ctx.accounts.state
            .total_staked
            .checked_mul(funding_rate as u64)
            .unwrap()
            .checked_div(100)
            .unwrap();
        mint_rewards(
            &ctx.accounts.state,
            &ctx.accounts.state_owner,
            &ctx.accounts.token_mint,
            &ctx.accounts.rewards_account,
            &ctx.accounts.token_program,
            reward_amount,
        )?;
        Ok(())
    }

    // 3️⃣ Vault Profit Sharing.
    // Distribute arbitrage profits from the vault to $DNT holders.
    pub fn distribute_arbitrage_profits(ctx: Context<DistributeProfits>) -> Result<()> {
        let total_profits = get_arbitrage_profits_from_vault()?;
        mint_rewards(
            &ctx.accounts.state,
            &ctx.accounts.state_owner,
            &ctx.accounts.token_mint,
            &ctx.accounts.rewards_account,
            &ctx.accounts.token_program,
            total_profits,
        )?;
        Ok(())
    }

    // 4️⃣ Liquidity Incentives for Market Makers.
    // Reward market makers who provide deep liquidity.
    pub fn reward_liquidity_providers(ctx: Context<RewardMakers>) -> Result<()> {
        let maker_volume = get_maker_trading_volume()?;
        let reward_amount = maker_volume.checked_div(1000).unwrap();
        mint_rewards(
            &ctx.accounts.state,
            &ctx.accounts.state_owner,
            &ctx.accounts.token_mint,
            &ctx.accounts.rewards_account,
            &ctx.accounts.token_program,
            reward_amount,
        )?;
        Ok(())
    }

    // 6️⃣ Automated Liquidations & Risk Management.
    // Liquidate traders if their loss exceeds the maximum allowed threshold.
    pub fn auto_liquidate(ctx: Context<Liquidate>) -> Result<()> {
        let user_position = get_user_position(ctx.accounts.user.key)?;
        if user_position.loss_percentage > MAX_ALLOWED_LOSS {
            force_close_position(&ctx)?;
            update_state_after_liquidation(&ctx)?;
        }
        Ok(())
    }

    // 8️⃣ Staked Voting (Governance).
    // Allow staked $DNT holders to vote on protocol risk parameters.
    pub fn vote_on_risk_params(ctx: Context<Vote>, new_threshold: u64) -> Result<()> {
        let total_votes = get_total_votes()?;
        let yes_votes = get_yes_votes()?;
        require!(
            yes_votes * 100 / total_votes >= 60,
            CustomError::NotEnoughVotes
        );
        let state = &mut ctx.accounts.state;
        state.allowed_delta_threshold = new_threshold;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Global State & User Stake Accounts
// -----------------------------------------------------------------------------

#[account]
pub struct State {
    pub bump: u8,
    pub total_staked: u64,
    pub last_update: i64,
    pub last_rebalance: i64,
    pub allowed_delta_threshold: u64,
}

#[account]
pub struct UserStake {
    pub amount: u64,
    pub last_update: i64,
}

// -----------------------------------------------------------------------------
// Accounts Contexts
// -----------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        seeds = [b"state", payer.key().as_ref()],
        bump,
        payer = payer,
        space = 8 + 1 + 8 + 8 + 8 + 8,
    )]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StakeAccounts<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    // Assume the user stake account is already initialized.
    #[account(mut, seeds = [b"user_stake", user.key().as_ref()], bump)]
    pub user_stake: Account<'info, UserStake>,
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: This account holds the user's $DNT tokens.
    #[account(mut, constraint = user_token_account.owner == user.key())]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"vault", state.key().as_ref()], bump)]
    pub vault_account: Account<'info, TokenAccount>,
    /// CHECK: Reference to state owner for PDA derivation.
    pub state_owner: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct MultiCollateralStakeAccounts<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    // Assume the user stake account is already initialized.
    #[account(mut, seeds = [b"user_stake", user.key().as_ref()], bump)]
    pub user_stake: Account<'info, UserStake>,
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: This account holds the user's collateral tokens.
    #[account(mut, constraint = user_token_account.owner == user.key())]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"vault", state.key().as_ref()], bump)]
    pub vault_account: Account<'info, TokenAccount>,
    /// CHECK: Reference to state owner.
    pub state_owner: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    #[account(mut, seeds = [b"user_stake", user.key().as_ref()], bump)]
    pub user_stake: Account<'info, UserStake>,
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: This account holds the user's $DNT tokens.
    #[account(mut, constraint = user_token_account.owner == user.key())]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"vault", state.key().as_ref()], bump)]
    pub vault_account: Account<'info, TokenAccount>,
    /// CHECK: Reference to state owner for PDA derivation.
    pub state_owner: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Rebalance<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    /// CHECK: Reference to state owner.
    pub state_owner: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct DistributeRewards<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    /// CHECK: Reference to state owner.
    pub state_owner: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub rewards_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct UpdateRewards<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    /// CHECK: Reference to state owner.
    pub state_owner: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub rewards_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct DistributeProfits<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    /// CHECK: Reference to state owner.
    pub state_owner: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub rewards_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RewardMakers<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    /// CHECK: Reference to state owner.
    pub state_owner: AccountInfo<'info>,
    #[account(mut)]
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub rewards_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    // Additional accounts for managing positions could be added here.
}

#[derive(Accounts)]
pub struct Vote<'info> {
    #[account(mut, seeds = [b"state", state_owner.key().as_ref()], bump = state.bump)]
    pub state: Account<'info, State>,
    /// CHECK: Reference to state owner.
    pub state_owner: AccountInfo<'info>,
}

// -----------------------------------------------------------------------------
// Helper Functions & Placeholders
// -----------------------------------------------------------------------------

fn get_funding_rate_from_oracle() -> Result<u64> {
    // Placeholder: Return a dummy funding rate (e.g., 5 basis points).
    Ok(5)
}

fn get_arbitrage_profits_from_vault() -> Result<u64> {
    // Placeholder: Return dummy arbitrage profits.
    Ok(1_000)
}

fn get_maker_trading_volume() -> Result<u64> {
    // Placeholder: Return dummy maker trading volume.
    Ok(5_000)
}

fn get_conversion_rate(_asset_type: u8) -> Result<u64> {
    // Placeholder: Assume a 1:1 conversion rate.
    Ok(1)
}

struct UserPosition {
    pub loss_percentage: u64,
}

fn get_user_position(_user: &Pubkey) -> Result<UserPosition> {
    // Placeholder: Return a dummy user position.
    Ok(UserPosition { loss_percentage: 10 })
}

fn force_close_position(_ctx: &Context<Liquidate>) -> Result<()> {
    // Placeholder for force-closing a user's position.
    Ok(())
}

fn update_state_after_liquidation(_ctx: &Context<Liquidate>) -> Result<()> {
    // Placeholder for updating state after liquidation.
    Ok(())
}

fn get_total_votes() -> Result<u64> {
    // Placeholder: Return total number of votes.
    Ok(100)
}

fn get_yes_votes() -> Result<u64> {
    // Placeholder: Return number of yes votes.
    Ok(70)
}

/// Helper function to mint rewards to a rewards account.
fn mint_rewards<'info>(
    state: &Account<'info, State>,
    state_owner: &AccountInfo<'info>,
    token_mint: &Account<'info, Mint>,
    rewards_account: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
    amount: u64,
) -> Result<()> {
    let seeds = &[b"state", state_owner.key.as_ref(), &[state.bump]];
    let signer = &[&seeds[..]];
    let cpi_accounts = MintTo {
        mint: token_mint.to_account_info(),
        to: rewards_account.to_account_info(),
        authority: state.to_account_info(),
    };
    token::mint_to(
        CpiContext::new_with_signer(token_program.to_account_info(), cpi_accounts, signer),
        amount,
    )?;
    Ok(())
}

// -----------------------------------------------------------------------------
// Error Codes
// -----------------------------------------------------------------------------

#[error_code]
pub enum CustomError {
    #[msg("Insufficient stake amount.")]
    InsufficientStake,
    #[msg("Early unstake is not allowed. Minimum staking duration not met.")]
    EarlyUnstakeNotAllowed,
    #[msg("Not enough votes for the proposal.")]
    NotEnoughVotes,
}
