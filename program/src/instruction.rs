//! Instruction types

#![allow(clippy::too_many_arguments)]

use crate::error::SwapError;
use crate::fees::Fees;
use solana_program::{program_error::ProgramError, program_pack::Pack};
use std::convert::TryInto;
use std::mem::size_of;

/// Initialize instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct Initialize {
    /// all swap fees
    pub fees: Fees,
}

/// DepositTokens instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct DepositTokens {
    /// Pool token amount to mint. token_a and token_b amount are set by
    /// the current exchange rate and size of the pool
    pub pool_token_amount: u64,
    /// Maximum token A amount to deposit, prevents excessive slippage
    pub maximum_token_a_amount: u64,
    /// Maximum token B amount to deposit, prevents excessive slippage
    pub maximum_token_b_amount: u64,
}

/// WithdrawTokens instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct WithdrawTokens {
    /// Pool token amount to burn. User receives an output of token a
    /// and b based on the percentage of the pool tokens that are returned
    pub pool_token_amount: u64,
    /// Minimum token A amount to receive, prevents excessive slippage
    pub minimum_token_a_amount: u64,
    /// Minimum token B amount to receive, prevents excessive slippage
    pub minimum_token_b_amount: u64,
}

/// Swap instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct Swap {
    /// SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    pub amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    pub minimum_amount_out: u64,
}

/// Instructions supported by the token swap program
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum SwapInstruction {
    /// Initialize a new swap
    ///
    /// 0. `[writable, signer]` New Token-swap to create.
    /// 1. `[]` swap authority derived from `create_program_address(&[Token-swap account])`
    /// 2. `[]` token_a Account. Must be non zero, owned by swap authority.
    /// 3. `[]` token_b Account. Must be non zero, owned by swap authority.
    /// 4. `[writable]` Pool Token Mint. Must be empty, owned by swap authority.
    /// 5. `[]` token_a Account to deposit trading fees. Must be empty, not
    /// owned by swap authority.
    /// 6. `[]` token_b Account to deposit trading fees. Must be empty, not
    /// owned by swap authority.
    /// 7. `[writable]` Pool Token Account to deposit the initial pool token
    /// supply. Must be empty, not owned by swap authority.
    /// 8. `[]` Token program id
    Initialize(Initialize),

    ///   Deposit both types of tokens into the pool.  The output is a "pool"
    ///   token representing ownership in the pool. Inputs are converted to
    ///   the current ratio.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` swap authority
    ///   2. `[signer]` user transfer authority
    ///   3. `[writable]` token_a user transfer authority can transfer amount,
    ///   4. `[writable]` token_b user transfer authority can transfer amount,
    ///   5. `[writable]` token_a Base Account to deposit into.
    ///   6. `[writable]` token_b Base Account to deposit into.
    ///   7. `[writable]` Pool MINT account, swap authority is the owner.
    ///   8. `[writable]` Pool Account to deposit the generated tokens, user is the owner.
    ///   9. `[]` Token program id
    DepositTokens(DepositTokens),

    ///   Withdraw both types of tokens from the pool at the current ratio, given
    ///   pool tokens.  The pool tokens are burned in exchange for an equivalent
    ///   amount of token A and B.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` swap authority
    ///   2. `[signer]` user transfer authority
    ///   3. `[writable]` Pool mint account, swap authority is the owner
    ///   4. `[writable]` SOURCE Pool account, amount is transferable by user transfer authority.
    ///   5. `[writable]` token_a Swap Account to withdraw FROM.
    ///   6. `[writable]` token_b Swap Account to withdraw FROM.
    ///   7. `[writable]` token_a user Account to credit.
    ///   8. `[writable]` token_b user Account to credit.
    ///   9. `[]` Token program id
    WithdrawTokens(WithdrawTokens),

    ///   Swap the tokens in the pool.
    ///
    ///   0. `[]` Token-swap
    ///   1. `[]` swap authority
    ///   2. `[signer]` user transfer authority
    ///   3. `[writable]` token_(A|B) SOURCE Account, amount is transferable by user transfer authority,
    ///   4. `[writable]` token_(A|B) Base Account to swap INTO.  Must be the SOURCE token.
    ///   5. `[writable]` token_(A|B) Base Account to swap FROM.  Must be the DESTINATION token.
    ///   6. `[writable]` token_(A|B) DESTINATION Account assigned to USER as the owner.
    ///   7. `[writable]` Fee account, to receive trading fees
    ///   8. `[]` Token program id
    Swap(Swap),
}

impl SwapInstruction {
    /// Unpacks a byte buffer into a [SwapInstruction](enum.SwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(SwapError::InvalidInstruction)?;
        Ok(match tag {
            0 => {
                if rest.len() == Fees::LEN {
                    let fees = Fees::unpack_unchecked(rest)?;
                    Self::Initialize(Initialize { fees })
                } else {
                    return Err(SwapError::InvalidInstruction.into());
                }
            }
            1 => {
                let (pool_token_amount, rest) = Self::unpack_u64(rest)?;
                let (maximum_token_a_amount, rest) = Self::unpack_u64(rest)?;
                let (maximum_token_b_amount, _rest) = Self::unpack_u64(rest)?;
                Self::DepositTokens(DepositTokens {
                    pool_token_amount,
                    maximum_token_a_amount,
                    maximum_token_b_amount,
                })
            }
            2 => {
                let (pool_token_amount, rest) = Self::unpack_u64(rest)?;
                let (minimum_token_a_amount, rest) = Self::unpack_u64(rest)?;
                let (minimum_token_b_amount, _rest) = Self::unpack_u64(rest)?;
                Self::WithdrawTokens(WithdrawTokens {
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                })
            }
            3 => {
                let (amount_in, rest) = Self::unpack_u64(rest)?;
                let (minimum_amount_out, _rest) = Self::unpack_u64(rest)?;
                Self::Swap(Swap {
                    amount_in,
                    minimum_amount_out,
                })
            }
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (amount, rest) = input.split_at(8);
            let amount = amount
                .get(..8)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(SwapError::InvalidInstruction)?;
            Ok((amount, rest))
        } else {
            Err(SwapError::InvalidInstruction.into())
        }
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match &*self {
            Self::Initialize(Initialize { fees }) => {
                buf.push(0);
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
            }
            Self::DepositTokens(DepositTokens {
                pool_token_amount,
                maximum_token_a_amount,
                maximum_token_b_amount,
            }) => {
                buf.push(1);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&maximum_token_a_amount.to_le_bytes());
                buf.extend_from_slice(&maximum_token_b_amount.to_le_bytes());
            }
            Self::WithdrawTokens(WithdrawTokens {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            }) => {
                buf.push(2);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
            }
            Self::Swap(Swap {
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(3);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
        }
        buf
    }
}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn pack_initialize() {
        let trade_fee_numerator: u64 = 1;
        let trade_fee_denominator: u64 = 4;
        let fees = Fees {
            trade_fee_numerator,
            trade_fee_denominator,
        };
        let check = SwapInstruction::Initialize(Initialize { fees });
        let packed = check.pack();
        let mut expect = vec![0u8];
        expect.extend_from_slice(&trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&trade_fee_denominator.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn pack_deposit() {
        let pool_token_amount: u64 = 5;
        let maximum_token_a_amount: u64 = 10;
        let maximum_token_b_amount: u64 = 20;
        let check = SwapInstruction::DepositTokens(DepositTokens {
            pool_token_amount,
            maximum_token_a_amount,
            maximum_token_b_amount,
        });
        let packed = check.pack();
        let mut expect = vec![1];
        expect.extend_from_slice(&pool_token_amount.to_le_bytes());
        expect.extend_from_slice(&maximum_token_a_amount.to_le_bytes());
        expect.extend_from_slice(&maximum_token_b_amount.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn pack_withdraw() {
        let pool_token_amount: u64 = 1212438012089;
        let minimum_token_a_amount: u64 = 102198761982612;
        let minimum_token_b_amount: u64 = 2011239855213;
        let check = SwapInstruction::WithdrawTokens(WithdrawTokens {
            pool_token_amount,
            minimum_token_a_amount,
            minimum_token_b_amount,
        });
        let packed = check.pack();
        let mut expect = vec![2];
        expect.extend_from_slice(&pool_token_amount.to_le_bytes());
        expect.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
        expect.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn pack_swap() {
        let amount_in: u64 = 2;
        let minimum_amount_out: u64 = 10;
        let check = SwapInstruction::Swap(Swap {
            amount_in,
            minimum_amount_out,
        });
        let packed = check.pack();
        let mut expect = vec![3];
        expect.extend_from_slice(&amount_in.to_le_bytes());
        expect.extend_from_slice(&minimum_amount_out.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }
}
