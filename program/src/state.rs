//! State transition types

use crate::fees::Fees;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

/// Program states.
#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct SwapState {
    /// Initialized state.
    pub is_initialized: bool,
    /// Bump seed used in program address.
    /// The program address is created deterministically with the bump seed,
    /// swap program id, and swap account pubkey.  This program address has
    /// authority over the swap's token A account, token B account, and pool
    /// token mint.
    pub bump_seed: u8,

    /// Program ID of the tokens being exchanged.
    pub token_program_id: Pubkey,

    /// Token A
    pub token_a: Pubkey,
    /// Token B
    pub token_b: Pubkey,

    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    pub pool_mint: Pubkey,

    /// Mint information for token A
    pub token_a_mint: Pubkey,
    /// Mint information for token B
    pub token_b_mint: Pubkey,

    /// token a account to receive trading and / or withdrawal fees
    pub token_a_fee_account: Pubkey,
    /// token b account to receive trading and / or withdrawal fees
    pub token_b_fee_account: Pubkey,

    /// All fee information
    pub fees: Fees,
}

/// SwapState representing access to program state
impl SwapState {
    /// Is the swap initialized, with data written to it
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Bump seed used to generate the program address / authority
    pub fn bump_seed(&self) -> u8 {
        self.bump_seed
    }

    /// Token program ID associated with the swap
    pub fn token_program_id(&self) -> &Pubkey {
        &self.token_program_id
    }

    /// Address of token A liquidity account
    pub fn token_a_account(&self) -> &Pubkey {
        &self.token_a
    }

    /// Address of token B liquidity account
    pub fn token_b_account(&self) -> &Pubkey {
        &self.token_b
    }

    /// Address of pool token mint
    pub fn pool_mint(&self) -> &Pubkey {
        &self.pool_mint
    }

    /// Address of token A mint
    pub fn token_a_mint(&self) -> &Pubkey {
        &self.token_a_mint
    }

    /// Address of token B mint
    pub fn token_b_mint(&self) -> &Pubkey {
        &self.token_b_mint
    }

    /// Address of token a fee account
    pub fn token_a_fee_account(&self) -> &Pubkey {
        &self.token_a_fee_account
    }

    /// Address of token b fee account
    pub fn token_b_fee_account(&self) -> &Pubkey {
        &self.token_b_fee_account
    }

    /// Fees associated with swap
    pub fn fees(&self) -> &Fees {
        &self.fees
    }
}

impl Sealed for SwapState {}
impl IsInitialized for SwapState {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for SwapState {
    const LEN: usize = 274;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, 274];
        let (
            is_initialized,
            bump_seed,
            token_program_id,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            token_a_fee_account,
            token_b_fee_account,
            fees,
        ) = mut_array_refs![output, 1, 1, 32, 32, 32, 32, 32, 32, 32, 32, 16];
        is_initialized[0] = self.is_initialized as u8;
        bump_seed[0] = self.bump_seed;
        token_program_id.copy_from_slice(self.token_program_id.as_ref());
        token_a.copy_from_slice(self.token_a.as_ref());
        token_b.copy_from_slice(self.token_b.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
        token_a_mint.copy_from_slice(self.token_a_mint.as_ref());
        token_b_mint.copy_from_slice(self.token_b_mint.as_ref());
        token_a_fee_account.copy_from_slice(self.token_a_fee_account.as_ref());
        token_b_fee_account.copy_from_slice(self.token_b_fee_account.as_ref());
        self.fees.pack_into_slice(&mut fees[..]);
    }

    /// Unpacks a byte buffer into a [SwapState](struct.SwapState.html).
    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, 274];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            bump_seed,
            token_program_id,
            token_a,
            token_b,
            pool_mint,
            token_a_mint,
            token_b_mint,
            token_a_fee_account,
            token_b_fee_account,
            fees,
        ) = array_refs![input, 1, 1, 32, 32, 32, 32, 32, 32, 32, 32, 16];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            bump_seed: bump_seed[0],
            token_program_id: Pubkey::new_from_array(*token_program_id),
            token_a: Pubkey::new_from_array(*token_a),
            token_b: Pubkey::new_from_array(*token_b),
            pool_mint: Pubkey::new_from_array(*pool_mint),
            token_a_mint: Pubkey::new_from_array(*token_a_mint),
            token_b_mint: Pubkey::new_from_array(*token_b_mint),
            token_a_fee_account: Pubkey::new_from_array(*token_a_fee_account),
            token_b_fee_account: Pubkey::new_from_array(*token_b_fee_account),
            fees: Fees::unpack_from_slice(fees)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FEES: Fees = Fees {
        trade_fee_numerator: 1,
        trade_fee_denominator: 4,
    };

    const TEST_BUMP_SEED: u8 = 255;
    const TEST_TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array([1u8; 32]);
    const TEST_TOKEN_A: Pubkey = Pubkey::new_from_array([2u8; 32]);
    const TEST_TOKEN_B: Pubkey = Pubkey::new_from_array([3u8; 32]);
    const TEST_POOL_MINT: Pubkey = Pubkey::new_from_array([4u8; 32]);
    const TEST_TOKEN_A_MINT: Pubkey = Pubkey::new_from_array([5u8; 32]);
    const TEST_TOKEN_B_MINT: Pubkey = Pubkey::new_from_array([6u8; 32]);
    const TEST_TOKEN_A_FEE_ACCOUNT: Pubkey = Pubkey::new_from_array([7u8; 32]);
    const TEST_TOKEN_B_FEE_ACCOUNT: Pubkey = Pubkey::new_from_array([8u8; 32]);

    #[test]
    fn swap_state_pack() {
        let swap_info = SwapState {
            is_initialized: true,
            bump_seed: TEST_BUMP_SEED,
            token_program_id: TEST_TOKEN_PROGRAM_ID,
            token_a: TEST_TOKEN_A,
            token_b: TEST_TOKEN_B,
            pool_mint: TEST_POOL_MINT,
            token_a_mint: TEST_TOKEN_A_MINT,
            token_b_mint: TEST_TOKEN_B_MINT,
            token_a_fee_account: TEST_TOKEN_A_FEE_ACCOUNT,
            token_b_fee_account: TEST_TOKEN_B_FEE_ACCOUNT,
            fees: TEST_FEES,
        };

        let mut packed = [0u8; SwapState::LEN];
        SwapState::pack_into_slice(&swap_info, &mut packed);
        let unpacked = SwapState::unpack(&packed).unwrap();
        assert_eq!(swap_info, unpacked);

        let mut packed = vec![1u8, TEST_BUMP_SEED];
        packed.extend_from_slice(&TEST_TOKEN_PROGRAM_ID.to_bytes());
        packed.extend_from_slice(&TEST_TOKEN_A.to_bytes());
        packed.extend_from_slice(&TEST_TOKEN_B.to_bytes());
        packed.extend_from_slice(&TEST_POOL_MINT.to_bytes());
        packed.extend_from_slice(&TEST_TOKEN_A_MINT.to_bytes());
        packed.extend_from_slice(&TEST_TOKEN_B_MINT.to_bytes());
        packed.extend_from_slice(&TEST_TOKEN_A_FEE_ACCOUNT.to_bytes());
        packed.extend_from_slice(&TEST_TOKEN_B_FEE_ACCOUNT.to_bytes());
        packed.extend_from_slice(&TEST_FEES.trade_fee_numerator.to_le_bytes());
        packed.extend_from_slice(&TEST_FEES.trade_fee_denominator.to_le_bytes());
        let unpacked = SwapState::unpack(&packed).unwrap();
        assert_eq!(swap_info, unpacked);

        let packed = [0u8; SwapState::LEN];
        let swap_info: SwapState = Default::default();
        let unpack_unchecked = SwapState::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, swap_info);
        let err = SwapState::unpack(&packed).unwrap_err();
        assert_eq!(err, ProgramError::UninitializedAccount);
    }
}
