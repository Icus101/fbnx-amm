use anchor_lang::prelude::*;
//use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token::{self, Burn, Mint, TokenAccount, Transfer, Token};

use crate::curve::{
    base::{SwapCurve, CurveType},
    calculator::{CurveCalculator, RoundDirection},
    fees::CurveFees,
};

use crate::state::*;
use crate::utils::*;
use crate::curve::{
     constant_product::ConstantProductCurve,
};
use crate::error::SwapError;

pub fn handler(
    ctx: Context<WithdrawAllTokenTypes>,
    pool_token_amount: u64,
    minimum_token_a_amount: u64,
    minimum_token_b_amount: u64,
) -> Result<()> {
    let amm = &mut ctx.accounts.amm;

    let curve = build_curve(&amm.curve).unwrap();
    let fees = build_fees(&amm.fees).unwrap();

    let calculator = curve.calculator;
    if !calculator.allows_deposits() {
        return Err(SwapError::UnsupportedCurveOperation.into());
    }

    check_accounts(
        amm,
        ctx.program_id,
        &amm.to_account_info(),
        &ctx.accounts.authority,
        &ctx.accounts.vault_token_a.to_account_info(),
        &ctx.accounts.vault_token_b.to_account_info(),
        &ctx.accounts.pool_mint.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        Some(&ctx.accounts.dest_token_a_info.to_account_info()),
        Some(&ctx.accounts.dest_token_b_info.to_account_info()),
        Some(&ctx.accounts.fee_account),
    )?;

    let withdraw_fee: u128 = if *ctx.accounts.fee_account.key == *ctx.accounts.source_info.to_account_info().key {
        // withdrawing from the fee account, don't assess withdraw fee
        0
    } else {
        fees.owner_withdraw_fee(u128::try_from(pool_token_amount).unwrap())
            .ok_or(SwapError::FeeCalculationFailure)?
    };
    let pool_token_amount = u128::try_from(pool_token_amount)
        .unwrap()
        .checked_sub(withdraw_fee)
        .ok_or(SwapError::CalculationFailure)?;

    let results = calculator
        .pool_tokens_to_trading_tokens(
            pool_token_amount,
            u128::try_from(ctx.accounts.pool_mint.supply).unwrap(),
            u128::try_from(ctx.accounts.vault_token_a.amount).unwrap(),
            u128::try_from(ctx.accounts.vault_token_b.amount).unwrap(),
            RoundDirection::Floor,
        )
        .ok_or(SwapError::ZeroTradingTokens)?;

    let token_a_amount = u64::try_from(results.token_a_amount).unwrap();
    let token_a_amount = std::cmp::min(ctx.accounts.vault_token_a.amount, token_a_amount);
    if token_a_amount < minimum_token_a_amount {
        return Err(SwapError::ExceededSlippage.into());
    }
    if token_a_amount == 0 && ctx.accounts.vault_token_a.amount != 0 {
        return Err(SwapError::ZeroTradingTokens.into());
    }
    let token_b_amount = u64::try_from(results.token_b_amount).unwrap();
    let token_b_amount = std::cmp::min(ctx.accounts.vault_token_b.amount, token_b_amount);
    if token_b_amount < minimum_token_b_amount {
        return Err(SwapError::ExceededSlippage.into());
    }
    if token_b_amount == 0 && ctx.accounts.vault_token_b.amount != 0 {
        return Err(SwapError::ZeroTradingTokens.into());
    }

    let seeds = &[&amm.to_account_info().key.to_bytes(), &[amm.bump_seed][..]];

    if withdraw_fee > 0 {
        token::transfer(
            ctx.accounts.into_transfer_to_fee_account_context(),
            u64::try_from(withdraw_fee).unwrap(),
        )?;
    }
    token::burn(
        ctx.accounts.into_burn_context(),
        u64::try_from(pool_token_amount).unwrap(),
    )?;

    if token_a_amount > 0 {
        token::transfer(
            ctx.accounts
                .into_transfer_to_token_a_context()
                .with_signer(&[&seeds[..]]),
            token_a_amount,
        )?;
    }
    if token_b_amount > 0 {
        token::transfer(
            ctx.accounts
                .into_transfer_to_token_b_context()
                .with_signer(&[&seeds[..]]),
            token_a_amount,
        )?;
    }
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawAllTokenTypes<'info> {
    #[account(
        mut,
        seeds = [b"amm".as_ref(),amm.token_a_mint.as_ref(),amm.token_b_account.as_ref()],
        bump,
     )]
    pub amm: Box<Account<'info, Amm>>,
    /// CHECK: Safe
    #[account(seeds=[b"authority".as_ref(), amm.key().as_ref()], bump)]
    pub authority: AccountInfo<'info>,
    /// CHECK: Safe
    #[account(mut)]
    pub owner: Signer<'info>,
    /// CHECK: Safe
    #[account(mut,
        token::mint = pool_mint.key()
    )]
    pub source_info: Box<Account<'info,TokenAccount>>,
    #[account(mut,
        constraint=vault_token_a.owner == authority.key(),
    )]
    pub vault_token_a: Account<'info, TokenAccount>,
    #[account(mut,
        constraint=vault_token_b.owner == authority.key(),
    )]
    pub vault_token_b: Account<'info, TokenAccount>,
    #[account(mut,
        mint::authority = authority.key()
    )]
    pub pool_mint: Box<Account<'info, Mint>>,
    /// CHECK: Safe
    #[account(mut,
        has_one = owner,
    )]
    pub dest_token_a_info: Account<'info,TokenAccount>,
    /// CHECK: Safe
    #[account(mut,
        has_one = owner
    )]
    pub dest_token_b_info: Account<'info,TokenAccount>,
    /// CHECK: Safe
    #[account(mut)]
    pub fee_account: AccountInfo<'info>,
    /// CHECK: Safe
    pub token_program: Program<'info,Token>,
}

impl<'info> WithdrawAllTokenTypes<'info> {
    fn into_transfer_to_fee_account_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.source_info.to_account_info().clone(),
            to: self.fee_account.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_burn_context(&self) -> CpiContext<'_, '_, '_, 'info, Burn<'info>> {
        let cpi_accounts = Burn {
            mint: self.pool_mint.to_account_info().clone(),
            from: self.source_info.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_transfer_to_token_a_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.vault_token_a.to_account_info().clone(),
            to: self.dest_token_a_info.to_account_info().clone(),
            authority: self.authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_transfer_to_token_b_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.vault_token_b.to_account_info().clone(),
            to: self.dest_token_b_info.to_account_info().clone(),
            authority: self.authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}

/// Build Curve object and Fee object
pub fn build_curve(curve_input: &CurveInput) -> Result<SwapCurve> {
    let curve_type = CurveType::try_from(curve_input.curve_type).unwrap();
    let culculator: Box<dyn CurveCalculator> = match curve_type {
        CurveType::ConstantProduct => Box::new(ConstantProductCurve {}),
        // CurveType::ConstantPrice => Box::new(ConstantPriceCurve {
        //     token_b_price: curve_input.curve_parameters,
        // }),
        // CurveType::Stable => Box::new(StableCurve {
        //     amp: curve_input.curve_parameters,
        // }),
        // CurveType::Offset => Box::new(OffsetCurve {
        //     token_b_offset: curve_input.curve_parameters,
        // }),
    };
    let curve = SwapCurve {
        curve_type: curve_type,
        calculator: culculator,
    };
    Ok(curve)
}

pub fn build_fees(fees_input: &FeesInput) -> Result<CurveFees> {
    let fees = CurveFees {
        trade_fee_numerator: fees_input.trade_fee_numerator,
        trade_fee_denominator: fees_input.trade_fee_denominator,
        owner_trade_fee_numerator: fees_input.owner_trade_fee_numerator,
        owner_trade_fee_denominator: fees_input.owner_trade_fee_denominator,
        owner_withdraw_fee_numerator: fees_input.owner_withdraw_fee_numerator,
        owner_withdraw_fee_denominator: fees_input.owner_withdraw_fee_denominator,
        host_fee_numerator: fees_input.host_fee_numerator,
        host_fee_denominator: fees_input.host_fee_denominator,
    };
    Ok(fees)
}
