//! Program state processor

use crate::constraints::{validate_fees, validate_supply};
use crate::{
    error::SwapError,
    fees::Fees,
    instruction::{DepositTokens, Initialize, Swap, SwapInstruction, WithdrawTokens},
    state::SwapState,
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
};
use std::convert::TryInto;

const INITIAL_SWAP_POOL_AMOUNT: u128 = 1_000_000_000;

/// Program state handler.
pub struct Processor {}

impl Processor {
    /// Unpacks a spl_token `Account`.
    pub fn unpack_token_account(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<spl_token::state::Account, SwapError> {
        if account_info.owner != token_program_id {
            Err(SwapError::IncorrectTokenProgramId)
        } else {
            spl_token::state::Account::unpack(&account_info.data.borrow())
                .map_err(|_| SwapError::ExpectedAccount)
        }
    }

    /// Unpacks a spl_token `Mint`.
    pub fn unpack_mint(
        account_info: &AccountInfo,
        token_program_id: &Pubkey,
    ) -> Result<spl_token::state::Mint, SwapError> {
        if account_info.owner != token_program_id {
            Err(SwapError::IncorrectTokenProgramId)
        } else {
            spl_token::state::Mint::unpack(&account_info.data.borrow())
                .map_err(|_| SwapError::ExpectedMint)
        }
    }

    /// Calculates the authority id by generating a program address.
    pub fn authority_id(
        program_id: &Pubkey,
        swap_info: &Pubkey,
        bump_seed: u8,
    ) -> Result<Pubkey, SwapError> {
        Pubkey::create_program_address(&[&swap_info.to_bytes()[..32], &[bump_seed]], program_id)
            .or(Err(SwapError::InvalidProgramAddress))
    }

    /// Issue a spl_token `Burn` instruction.
    pub fn token_burn<'a>(
        token_program: AccountInfo<'a>, // should be pool token program address
        burn_account: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        authority_id: AccountInfo<'a>,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            authority_id.key,
            &[],
            amount,
        )?;

        invoke_signed(&ix, &[burn_account, mint, authority_id, token_program], &[])
    }

    /// Issue a spl_token `MintTo` instruction.
    pub fn token_mint_to<'a>(
        swap_info: &Pubkey,
        token_program: AccountInfo<'a>, // should be pool token program address
        mint: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority_id: AccountInfo<'a>,
        bump_seed: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [&swap_info.to_bytes()[..32], &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority_id.key,
            &[],
            amount,
        )?;

        invoke_signed(
            &ix,
            &[mint, destination, authority_id, token_program],
            signers,
        )
    }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer<'a>(
        swap_info: &Pubkey,
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>, // Should be token A or token B token address owned by authority_id
        destination: AccountInfo<'a>,
        authority_id: AccountInfo<'a>,
        bump_seed: u8, // put this, only when the token is withdrawn from the program's token address
        amount: u64,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [&swap_info.to_bytes()[..32], &[bump_seed]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority_id.key,
            &[],
            amount,
        )?;
        invoke_signed(
            &ix,
            &[source, destination, authority_id, token_program],
            signers,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn check_accounts(
        swap_state: &SwapState,
        program_id: &Pubkey,
        swap_info: &AccountInfo,
        authority_info: &AccountInfo,
        token_a_info: &AccountInfo,
        token_b_info: &AccountInfo,
        pool_mint_info: &AccountInfo,
        token_program_info: &AccountInfo,
        user_token_a_info: Option<&AccountInfo>,
        user_token_b_info: Option<&AccountInfo>,
        token_a_fee_account_info: Option<&AccountInfo>,
        token_b_fee_account_info: Option<&AccountInfo>,
    ) -> ProgramResult {
        if swap_info.owner != program_id {
            return Err(ProgramError::IncorrectProgramId);
        }
        if *authority_info.key
            != Self::authority_id(program_id, swap_info.key, swap_state.bump_seed())?
        {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        if *token_a_info.key != *swap_state.token_a_account() {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if *token_b_info.key != *swap_state.token_b_account() {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if *pool_mint_info.key != *swap_state.pool_mint() {
            return Err(SwapError::IncorrectPoolMint.into());
        }
        if *token_program_info.key != *swap_state.token_program_id() {
            return Err(SwapError::IncorrectTokenProgramId.into());
        }
        if let Some(user_token_a_info) = user_token_a_info {
            if token_a_info.key == user_token_a_info.key {
                return Err(SwapError::InvalidInput.into());
            }
        }
        if let Some(user_token_b_info) = user_token_b_info {
            if token_b_info.key == user_token_b_info.key {
                return Err(SwapError::InvalidInput.into());
            }
        }
        if let Some(token_a_fee_account_info) = token_a_fee_account_info {
            if *token_a_fee_account_info.key != *swap_state.token_a_fee_account() {
                return Err(SwapError::IncorrectFeeAccount.into());
            }
        }
        if let Some(token_b_fee_account_info) = token_b_fee_account_info {
            if *token_b_fee_account_info.key != *swap_state.token_b_fee_account() {
                return Err(SwapError::IncorrectFeeAccount.into());
            }
        }
        Ok(())
    }

    /// Processes an [Initialize](enum.Instruction.html).
    pub fn process_initialize(
        program_id: &Pubkey,
        fees: Fees,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let token_a_fee_account_info = next_account_info(account_info_iter)?;
        let token_b_fee_account_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_program_id = *token_program_info.key;

        // check the swap_info already in use
        if match SwapState::unpack(&swap_info.data.borrow()) {
            Ok(swap) => swap.is_initialized(),
            Err(_) => false,
        } {
            return Err(SwapError::AlreadyInUse.into());
        }

        let (swap_authority, bump_seed) =
            Pubkey::find_program_address(&[&swap_info.key.to_bytes()], program_id);
        if *authority_info.key != swap_authority {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        let token_a = Self::unpack_token_account(token_a_info, &token_program_id)?;
        let token_b = Self::unpack_token_account(token_b_info, &token_program_id)?;
        let token_a_fee_account =
            Self::unpack_token_account(token_a_fee_account_info, &token_program_id)?;
        let token_b_fee_account =
            Self::unpack_token_account(token_b_fee_account_info, &token_program_id)?;
        let destination = Self::unpack_token_account(destination_info, &token_program_id)?;
        let pool_mint = Self::unpack_mint(pool_mint_info, &token_program_id)?;
        if *authority_info.key != token_a.owner {
            return Err(SwapError::InvalidOwner.into());
        }
        if *authority_info.key != token_b.owner {
            return Err(SwapError::InvalidOwner.into());
        }
        if *authority_info.key == destination.owner {
            return Err(SwapError::InvalidOutputOwner.into());
        }
        if *authority_info.key == token_a_fee_account.owner {
            return Err(SwapError::InvalidOutputOwner.into());
        }
        if *authority_info.key == token_b_fee_account.owner {
            return Err(SwapError::InvalidOutputOwner.into());
        }
        if COption::Some(*authority_info.key) != pool_mint.mint_authority {
            return Err(SwapError::InvalidOwner.into());
        }

        if token_a.mint == token_b.mint {
            return Err(SwapError::RepeatedMint.into());
        }

        // Both of the token amount should be non-zero
        validate_supply(token_a.amount, token_b.amount)?;

        if token_a.delegate.is_some() {
            return Err(SwapError::InvalidDelegate.into());
        }
        if token_b.delegate.is_some() {
            return Err(SwapError::InvalidDelegate.into());
        }
        if token_a.close_authority.is_some() {
            return Err(SwapError::InvalidCloseAuthority.into());
        }
        if token_b.close_authority.is_some() {
            return Err(SwapError::InvalidCloseAuthority.into());
        }
        if token_a.mint != token_a_fee_account.mint {
            return Err(SwapError::IncorrectFeeAccount.into());
        }
        if token_b.mint != token_b_fee_account.mint {
            return Err(SwapError::IncorrectFeeAccount.into());
        }

        if pool_mint.supply != 0 {
            return Err(SwapError::InvalidSupply.into());
        }
        if pool_mint.freeze_authority.is_some() {
            return Err(SwapError::InvalidFreezeAuthority.into());
        }

        fees.validate()?;
        validate_fees(&fees)?;

        let initial_amount = INITIAL_SWAP_POOL_AMOUNT;

        Self::token_mint_to(
            swap_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            destination_info.clone(),
            authority_info.clone(),
            bump_seed,
            to_u64(initial_amount)?,
        )?;

        let swap_state = SwapState {
            is_initialized: true,
            bump_seed,
            token_program_id,
            token_a: *token_a_info.key,
            token_b: *token_b_info.key,
            pool_mint: *pool_mint_info.key,
            token_a_mint: token_a.mint,
            token_b_mint: token_b.mint,
            token_a_fee_account: *token_a_fee_account_info.key,
            token_b_fee_account: *token_b_fee_account_info.key,
            fees,
        };
        SwapState::pack(swap_state, &mut swap_info.data.borrow_mut())?;
        Ok(())
    }

    /// Processes an [DepositTokens](enum.Instruction.html).
    pub fn process_deposit_tokens(
        program_id: &Pubkey,
        pool_token_amount: u64,
        maximum_token_a_amount: u64,
        maximum_token_b_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let user_transfer_authority_info = next_account_info(account_info_iter)?;
        let source_a_info = next_account_info(account_info_iter)?;
        let source_b_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let dest_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let swap_state = SwapState::unpack(&swap_info.data.borrow())?;
        Self::check_accounts(
            &swap_state,
            program_id,
            swap_info,
            authority_info,
            token_a_info,
            token_b_info,
            pool_mint_info,
            token_program_info,
            Some(source_a_info),
            Some(source_b_info),
            None,
            None,
        )?;

        let token_a = Self::unpack_token_account(token_a_info, swap_state.token_program_id())?;
        let token_b = Self::unpack_token_account(token_b_info, swap_state.token_program_id())?;
        let pool_mint = Self::unpack_mint(pool_mint_info, swap_state.token_program_id())?;
        let current_pool_mint_supply = to_u128(pool_mint.supply)?;
        let (pool_token_amount, pool_mint_supply) = if current_pool_mint_supply > 0 {
            (to_u128(pool_token_amount)?, current_pool_mint_supply)
        } else {
            (INITIAL_SWAP_POOL_AMOUNT, INITIAL_SWAP_POOL_AMOUNT)
        };

        // let token_a_amount = token_a.amount * pool_token_amount / pool_token_supply
        // let token_b_amount = token_b.amount * pool_token_amount / pool_token_supply
        let token_a_amount = to_u128(token_a.amount)? * pool_token_amount / pool_mint_supply;
        let token_b_amount = to_u128(token_b.amount)? * pool_token_amount / pool_mint_supply;

        let token_a_amount = to_u64(token_a_amount)?;
        if token_a_amount > maximum_token_a_amount {
            return Err(SwapError::ExceededSlippage.into());
        }
        if token_a_amount == 0 {
            return Err(SwapError::ZeroTradingTokens.into());
        }
        let token_b_amount = to_u64(token_b_amount)?;
        if token_b_amount > maximum_token_b_amount {
            return Err(SwapError::ExceededSlippage.into());
        }
        if token_b_amount == 0 {
            return Err(SwapError::ZeroTradingTokens.into());
        }

        let pool_token_amount = to_u64(pool_token_amount)?;

        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_a_info.clone(),
            token_a_info.clone(),
            user_transfer_authority_info.clone(),
            swap_state.bump_seed(),
            token_a_amount,
        )?;
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_b_info.clone(),
            token_b_info.clone(),
            user_transfer_authority_info.clone(),
            swap_state.bump_seed(),
            token_b_amount,
        )?;
        Self::token_mint_to(
            swap_info.key,
            token_program_info.clone(),
            pool_mint_info.clone(),
            dest_info.clone(),
            authority_info.clone(),
            swap_state.bump_seed(),
            pool_token_amount,
        )?;

        Ok(())
    }

    /// Processes an [WithdrawTokens](enum.Instruction.html).
    pub fn process_withdraw_tokens(
        program_id: &Pubkey,
        pool_token_amount: u64,
        minimum_token_a_amount: u64,
        minimum_token_b_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let user_transfer_authority_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let dest_token_a_info = next_account_info(account_info_iter)?;
        let dest_token_b_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let swap_state = SwapState::unpack(&swap_info.data.borrow())?;
        Self::check_accounts(
            &swap_state,
            program_id,
            swap_info,
            authority_info,
            token_a_info,
            token_b_info,
            pool_mint_info,
            token_program_info,
            Some(dest_token_a_info),
            Some(dest_token_b_info),
            None,
            None,
        )?;

        let token_a = Self::unpack_token_account(token_a_info, swap_state.token_program_id())?;
        let token_b = Self::unpack_token_account(token_b_info, swap_state.token_program_id())?;
        let pool_mint = Self::unpack_mint(pool_mint_info, swap_state.token_program_id())?;

        let pool_token_amount = to_u128(pool_token_amount)?;
        let pool_mint_supply = to_u128(pool_mint.supply)?;

        // let token_a_amount = token_a.amount * pool_token_amount / pool_token_supply
        // let token_b_amount = token_b.amount * pool_token_amount / pool_token_supply
        let token_a_amount = to_u128(token_a.amount)? * pool_token_amount / pool_mint_supply;
        let token_b_amount = to_u128(token_b.amount)? * pool_token_amount / pool_mint_supply;

        let token_a_amount = to_u64(token_a_amount)?;
        let token_a_amount = std::cmp::min(token_a.amount, token_a_amount);
        if token_a_amount < minimum_token_a_amount {
            return Err(SwapError::ExceededSlippage.into());
        }
        if token_a_amount == 0 && token_a.amount != 0 {
            return Err(SwapError::ZeroTradingTokens.into());
        }
        let token_b_amount = to_u64(token_b_amount)?;
        let token_b_amount = std::cmp::min(token_b.amount, token_b_amount);
        if token_b_amount < minimum_token_b_amount {
            return Err(SwapError::ExceededSlippage.into());
        }
        if token_b_amount == 0 && token_b.amount != 0 {
            return Err(SwapError::ZeroTradingTokens.into());
        }

        Self::token_burn(
            token_program_info.clone(),
            source_info.clone(),
            pool_mint_info.clone(),
            user_transfer_authority_info.clone(),
            to_u64(pool_token_amount)?,
        )?;

        if token_a_amount > 0 {
            Self::token_transfer(
                swap_info.key,
                token_program_info.clone(),
                token_a_info.clone(),
                dest_token_a_info.clone(),
                authority_info.clone(),
                swap_state.bump_seed(),
                token_a_amount,
            )?;
        }
        if token_b_amount > 0 {
            Self::token_transfer(
                swap_info.key,
                token_program_info.clone(),
                token_b_info.clone(),
                dest_token_b_info.clone(),
                authority_info.clone(),
                swap_state.bump_seed(),
                token_b_amount,
            )?;
        }
        Ok(())
    }

    /// Processes an [Swap](enum.Instruction.html).
    pub fn process_swap(
        program_id: &Pubkey,
        amount_in: u64,
        minimum_amount_out: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let swap_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let user_transfer_authority_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let swap_source_info = next_account_info(account_info_iter)?;
        let swap_destination_info = next_account_info(account_info_iter)?;
        let destination_info = next_account_info(account_info_iter)?;
        let fee_account_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        if swap_info.owner != program_id {
            return Err(ProgramError::IncorrectProgramId);
        }
        let swap_state = SwapState::unpack(&swap_info.data.borrow())?;

        if *authority_info.key
            != Self::authority_id(program_id, swap_info.key, swap_state.bump_seed())?
        {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        if !(*swap_source_info.key == *swap_state.token_a_account()
            || *swap_source_info.key == *swap_state.token_b_account())
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if !(*swap_destination_info.key == *swap_state.token_a_account()
            || *swap_destination_info.key == *swap_state.token_b_account())
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if *swap_source_info.key == *swap_destination_info.key {
            return Err(SwapError::InvalidInput.into());
        }
        if swap_source_info.key == source_info.key {
            // source_info should be user's not program one
            return Err(SwapError::InvalidInput.into());
        }
        if swap_destination_info.key == destination_info.key {
            // destination_info should be user's not program one
            return Err(SwapError::InvalidInput.into());
        }
        if *fee_account_info.key != *swap_state.token_a_fee_account()
            && *fee_account_info.key != *swap_state.token_b_fee_account()
        {
            return Err(SwapError::IncorrectFeeAccount.into());
        }
        if *token_program_info.key != *swap_state.token_program_id() {
            return Err(SwapError::IncorrectTokenProgramId.into());
        }

        let source_account =
            Self::unpack_token_account(swap_source_info, swap_state.token_program_id())?;
        let dest_account =
            Self::unpack_token_account(swap_destination_info, swap_state.token_program_id())?;
        let fee_amount =
            Self::unpack_token_account(fee_account_info, swap_state.token_program_id())?;

        if fee_amount.mint != source_account.mint {
            return Err(SwapError::IncorrectFeeAccount.into());
        }

        // charge trading fees
        let amount_in = to_u128(amount_in)?;
        let trading_fees = swap_state.fees().trading_fee(amount_in).unwrap_or(0u128);
        let amount_in = amount_in - trading_fees;

        let swap_token_source_amount = to_u128(source_account.amount)?;
        let swap_token_dest_amount = to_u128(dest_account.amount)?;

        // x * y = k
        // (x + amount_in) * (y - amount_out) = k
        // amount_out = y - k / (x + amount_in)
        //             = y - x * y / (x + amount_in)
        let amount_out = swap_token_dest_amount
            - swap_token_source_amount * swap_token_dest_amount
                / (swap_token_source_amount + amount_in);
        if amount_out < to_u128(minimum_amount_out)? {
            return Err(SwapError::ExceededSlippage.into());
        }

        // transfer source token from user to program
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_info.clone(),
            swap_source_info.clone(),
            user_transfer_authority_info.clone(),
            swap_state.bump_seed(),
            to_u64(amount_in)?,
        )?;

        // transfer dest token from program to user
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            swap_destination_info.clone(),
            destination_info.clone(),
            authority_info.clone(),
            swap_state.bump_seed(),
            to_u64(amount_out)?,
        )?;

        // transfer trading fees
        Self::token_transfer(
            swap_info.key,
            token_program_info.clone(),
            source_info.clone(),
            fee_account_info.clone(),
            user_transfer_authority_info.clone(),
            swap_state.bump_seed(),
            to_u64(trading_fees)?,
        )?;

        Ok(())
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = SwapInstruction::unpack(input)?;
        match instruction {
            SwapInstruction::Initialize(Initialize { fees }) => {
                msg!("Instruction: Init");
                Self::process_initialize(program_id, fees, accounts)
            }
            SwapInstruction::DepositTokens(DepositTokens {
                pool_token_amount,
                maximum_token_a_amount,
                maximum_token_b_amount,
            }) => {
                msg!("Instruction: DepositTokens");
                Self::process_deposit_tokens(
                    program_id,
                    pool_token_amount,
                    maximum_token_a_amount,
                    maximum_token_b_amount,
                    accounts,
                )
            }
            SwapInstruction::WithdrawTokens(WithdrawTokens {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            }) => {
                msg!("Instruction: WithdrawTokens");
                Self::process_withdraw_tokens(
                    program_id,
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                    accounts,
                )
            }
            SwapInstruction::Swap(Swap {
                amount_in,
                minimum_amount_out,
            }) => {
                msg!("Instruction: Swap");
                Self::process_swap(program_id, amount_in, minimum_amount_out, accounts)
            }
        }
    }
}

fn to_u128(val: u64) -> Result<u128, SwapError> {
    val.try_into().map_err(|_| SwapError::ConversionFailure)
}

fn to_u64(val: u128) -> Result<u64, SwapError> {
    val.try_into().map_err(|_| SwapError::ConversionFailure)
}
