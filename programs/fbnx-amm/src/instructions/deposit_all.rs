use anchor_lang::prelude::*;
//use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token::{self, Mint, MintTo, TokenAccount, Transfer, Token};
//pub mod curve;


use crate::curve::{
    base::{SwapCurve,CurveType},
    calculator::{CurveCalculator, RoundDirection},
    fees::CurveFees,
};

use crate::state::*;
use crate::utils::*;
use crate::error::SwapError;

use crate::curve::{
     constant_product::ConstantProductCurve,
};

pub fn handler(
    ctx: Context<DepositAllTokenTypes>,
    pool_token_amount: u64,
    maximum_token_a_amount: u64,
    maximum_token_b_amount: u64,
) -> Result<()> {
    let amm = &mut ctx.accounts.amm;

    let curve = build_curve(&amm.curve).unwrap();
    let calculator = curve.calculator;
    if !calculator.allows_deposits() {
        return Err(SwapError::UnsupportedCurveOperation.into());
    }

    check_accounts(
        amm,
        ctx.program_id,
        &amm.to_account_info(),
        &ctx.accounts.pool_authority,
        &ctx.accounts.vault_token_a.to_account_info(),
        &ctx.accounts.vault_token_b.to_account_info(),
        &ctx.accounts.pool_mint.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        Some(&ctx.accounts.source_a_info.to_account_info()),
        Some(&ctx.accounts.source_b_info.to_account_info()),
        None,
    )?;

    let current_pool_mint_supply = u128::try_from(ctx.accounts.pool_mint.supply).unwrap();
    let (pool_token_amount, pool_mint_supply) = if current_pool_mint_supply > 0 {
        (
            u128::try_from(pool_token_amount).unwrap(),
            current_pool_mint_supply,
        )
    } else {
        (calculator.new_pool_supply(), calculator.new_pool_supply())
    };

    let results = calculator
        .pool_tokens_to_trading_tokens(
            pool_token_amount,
            pool_mint_supply,
            u128::try_from(ctx.accounts.vault_token_a.amount).unwrap(),
            u128::try_from(ctx.accounts.vault_token_b.amount).unwrap(),
            RoundDirection::Ceiling,
        )
        .ok_or(SwapError::ZeroTradingTokens)?;
    let token_a_amount = u64::try_from(results.token_a_amount).unwrap();
    if token_a_amount > maximum_token_a_amount {
        return Err(SwapError::ExceededSlippage.into());
    }
    if token_a_amount == 0 {
        return Err(SwapError::ZeroTradingTokens.into());
    }
    let token_b_amount = u64::try_from(results.token_b_amount).unwrap();
    if token_b_amount > maximum_token_b_amount {
        return Err(SwapError::ExceededSlippage.into());
    }
    if token_b_amount == 0 {
        return Err(SwapError::ZeroTradingTokens.into());
    }

    let pool_token_amount = u64::try_from(pool_token_amount).unwrap();

    let seeds = &[&amm.to_account_info().key.to_bytes(), &[amm.bump_seed][..]];

    token::transfer(
        ctx.accounts
            .into_transfer_to_token_a_context()
            .with_signer(&[&seeds[..]]),
        token_a_amount,
    )?;

    token::transfer(
        ctx.accounts
            .into_transfer_to_token_b_context()
            .with_signer(&[&seeds[..]]),
        token_b_amount,
    )?;

    token::mint_to(
        ctx.accounts
            .into_mint_to_context()
            .with_signer(&[&seeds[..]]),
        u64::try_from(pool_token_amount).unwrap(),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct DepositAllTokenTypes<'info> {
    #[account(mut)]
    pub amm: Box<Account<'info, Amm>>,
    /// CHECK: Safe
    #[account(seeds=[b"authority".as_ref(), amm.key().as_ref()], bump)]
    pub pool_authority: AccountInfo<'info>,
    /// CHECK: Safe
    // #[account(signer)]
    // pub user_transfer_authority_info: AccountInfo<'info>,
    /// CHECK: Safe
    #[account(mut,
        has_one = owner
    )]
    pub source_a_info: Account<'info,TokenAccount>,
    /// CHECK: Safe
    #[account(mut,
        has_one = owner
    )]
    pub source_b_info: Account<'info,TokenAccount>,
    #[account(mut,
        constraint=vault_token_a.owner == pool_authority.key(),
    )]
    pub vault_token_a: Account<'info, TokenAccount>,
    #[account(mut,
        constraint=vault_token_b.owner == pool_authority.key(),
    )]
    pub vault_token_b: Account<'info, TokenAccount>,
    #[account(mut,
        mint::authority = pool_authority,
    )]
    pub pool_mint:Box< Account<'info, Mint>>,
    /// CHECK: Safe
    #[account(
        init_if_needed,
        payer = owner,
        token::mint = pool_mint,
        token::authority = owner
    )]
    pub destination: Box<Account<'info,TokenAccount>>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info,Token>,
    pub system_program : Program<'info,System>
}

impl<'info> DepositAllTokenTypes<'info> {
    fn into_transfer_to_token_a_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.source_a_info.to_account_info().clone(),
            to: self.vault_token_a.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_transfer_to_token_b_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.source_b_info.to_account_info().clone(),
            to: self.vault_token_b.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_mint_to_context(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.pool_mint.to_account_info().clone(),
            to: self.destination.to_account_info().clone(),
            authority: self.pool_authority.clone(),
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