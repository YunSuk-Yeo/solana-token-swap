import assert from 'assert';
import BN from 'bn.js';
import {Buffer} from 'buffer';
import * as BufferLayout from '@solana/buffer-layout';
import type {
  ConfirmOptions,
  Connection,
  TransactionSignature,
} from '@solana/web3.js';
import {
  Account,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';

import * as Layout from './layout';
import {loadAccount} from './util/account';

export const TOKEN_SWAP_PROGRAM_ID: PublicKey = new PublicKey(
  'SwapsVeCiPHMUAtzQWZw7RjsKjgCjhwU55QGu4U1Szw',
);

/**
 * Some amount of tokens
 */
export class Numberu64 extends BN {
  /**
   * Convert to Buffer representation
   */
  toBuffer(): Buffer {
    const a = super.toArray().reverse();
    const b = Buffer.from(a);
    if (b.length === 8) {
      return b;
    }
    assert(b.length < 8, 'Numberu64 too large');

    const zeroPad = Buffer.alloc(8);
    b.copy(zeroPad);
    return zeroPad;
  }

  /**
   * Construct a Numberu64 from Buffer representation
   */
  static fromBuffer(buffer: Buffer): Numberu64 {
    assert(buffer.length === 8, `Invalid buffer length: ${buffer.length}`);
    return new Numberu64(
      [...buffer]
        .reverse()
        .map(i => `00${i.toString(16)}`.slice(-2))
        .join(''),
      16,
    );
  }
}

export const TokenSwapLayout = BufferLayout.struct([
  BufferLayout.u8('isInitialized'),
  BufferLayout.u8('bumpSeed'),
  Layout.publicKey('tokenProgramId'),
  Layout.publicKey('tokenAccountA'),
  Layout.publicKey('tokenAccountB'),
  Layout.publicKey('tokenPool'),
  Layout.publicKey('mintA'),
  Layout.publicKey('mintB'),
  Layout.publicKey('tokenFeeAccountA'),
  Layout.publicKey('tokenFeeAccountB'),
  Layout.uint64('tradeFeeNumerator'),
  Layout.uint64('tradeFeeDenominator'),
]);

/**
 * A program to exchange tokens against a pool of liquidity
 */
export class TokenSwap {
  /**
   * Create a Token object attached to the specific token
   *
   * @param connection The connection to use
   * @param tokenSwap The token swap account
   * @param swapProgramId The program ID of the token-swap program
   * @param tokenProgramId The program ID of the token program
   * @param poolToken The pool token
   * @param tokenFeeAccountA The token A fee account
   * @param tokenFeeAccountB The token B fee account
   * @param authority The authority over the swap and accounts
   * @param tokenAccountA The token swap's Token A account
   * @param tokenAccountB The token swap's Token B account
   * @param mintA The mint of Token A
   * @param mintB The mint of Token B
   * @param tradeFeeNumerator The trade fee numerator
   * @param tradeFeeDenominator The trade fee denominator
   * @param payer Pays for the transaction
   */
  constructor(
    private connection: Connection,
    public tokenSwap: PublicKey,
    public swapProgramId: PublicKey,
    public tokenProgramId: PublicKey,
    public poolToken: PublicKey,
    public tokenFeeAccountA: PublicKey,
    public tokenFeeAccountB: PublicKey,
    public authority: PublicKey,
    public tokenAccountA: PublicKey,
    public tokenAccountB: PublicKey,
    public mintA: PublicKey,
    public mintB: PublicKey,
    public tradeFeeNumerator: Numberu64,
    public tradeFeeDenominator: Numberu64,
    public payer: Account,
  ) {
    this.connection = connection;
    this.tokenSwap = tokenSwap;
    this.swapProgramId = swapProgramId;
    this.tokenProgramId = tokenProgramId;
    this.poolToken = poolToken;
    this.tokenFeeAccountA = tokenFeeAccountA;
    this.tokenFeeAccountB = tokenFeeAccountB;
    this.authority = authority;
    this.tokenAccountA = tokenAccountA;
    this.tokenAccountB = tokenAccountB;
    this.mintA = mintA;
    this.mintB = mintB;
    this.tradeFeeNumerator = tradeFeeNumerator;
    this.tradeFeeDenominator = tradeFeeDenominator;
    this.payer = payer;
  }

  /**
   * Get the minimum balance for the token swap account to be rent exempt
   *
   * @return Number of lamports required
   */
  static async getMinBalanceRentForExemptTokenSwap(
    connection: Connection,
  ): Promise<number> {
    return await connection.getMinimumBalanceForRentExemption(
      TokenSwapLayout.span,
    );
  }

  static createInitSwapInstruction(
    tokenSwapAccount: Account,
    authority: PublicKey,
    tokenAccountA: PublicKey,
    tokenAccountB: PublicKey,
    tokenPool: PublicKey,
    tokenFeeAccountA: PublicKey,
    tokenFeeAccountB: PublicKey,
    tokenAccountPool: PublicKey,
    tokenProgramId: PublicKey,
    swapProgramId: PublicKey,
    tradeFeeNumerator: number,
    tradeFeeDenominator: number,
  ): TransactionInstruction {
    const keys = [
      {pubkey: tokenSwapAccount.publicKey, isSigner: false, isWritable: true},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: tokenAccountA, isSigner: false, isWritable: false},
      {pubkey: tokenAccountB, isSigner: false, isWritable: false},
      {pubkey: tokenPool, isSigner: false, isWritable: true},
      {pubkey: tokenFeeAccountA, isSigner: false, isWritable: false},
      {pubkey: tokenFeeAccountB, isSigner: false, isWritable: false},
      {pubkey: tokenAccountPool, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    const commandDataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      BufferLayout.nu64('tradeFeeNumerator'),
      BufferLayout.nu64('tradeFeeDenominator'),
    ]);
    let data = Buffer.alloc(1024);

    {
      const encodeLength = commandDataLayout.encode(
        {
          instruction: 0, // InitializeSwap instruction
          tradeFeeNumerator,
          tradeFeeDenominator,
        },
        data,
      );
      data = data.slice(0, encodeLength);
    }
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  static async loadTokenSwap(
    connection: Connection,
    address: PublicKey,
    programId: PublicKey,
    payer: Account,
  ): Promise<TokenSwap> {
    const data = await loadAccount(connection, address, programId);
    const tokenSwapData = TokenSwapLayout.decode(data);
    if (!tokenSwapData.isInitialized) {
      throw new Error(`Invalid token swap state`);
    }

    const [authority] = await PublicKey.findProgramAddress(
      [address.toBuffer()],
      programId,
    );

    const poolToken = new PublicKey(tokenSwapData.tokenPool);
    const tokenAccountA = new PublicKey(tokenSwapData.tokenAccountA);
    const tokenAccountB = new PublicKey(tokenSwapData.tokenAccountB);
    const mintA = new PublicKey(tokenSwapData.mintA);
    const mintB = new PublicKey(tokenSwapData.mintB);
    const tokenFeeAccountA = new PublicKey(tokenSwapData.tokenFeeAccountA);
    const tokenFeeAccountB = new PublicKey(tokenSwapData.tokenFeeAccountB);
    const tokenProgramId = new PublicKey(tokenSwapData.tokenProgramId);

    const tradeFeeNumerator = Numberu64.fromBuffer(
      tokenSwapData.tradeFeeNumerator,
    );
    const tradeFeeDenominator = Numberu64.fromBuffer(
      tokenSwapData.tradeFeeDenominator,
    );

    return new TokenSwap(
      connection,
      address,
      programId,
      tokenProgramId,
      poolToken,
      tokenFeeAccountA,
      tokenFeeAccountB,
      authority,
      tokenAccountA,
      tokenAccountB,
      mintA,
      mintB,
      tradeFeeNumerator,
      tradeFeeDenominator,
      payer,
    );
  }

  /**
   * Create a new Token Swap
   *
   * @param connection The connection to use
   * @param payer Pays for the transaction
   * @param tokenSwapAccount The token swap account
   * @param authority The authority over the swap and accounts
   * @param tokenAccountA: The token swap's Token A account
   * @param tokenAccountB: The token swap's Token B account
   * @param poolToken The pool token
   * @param tokenFeeAccountA The token A fee account
   * @param tokenFeeAccountB The token B fee account
   * @param tokenAccountPool The token swap's pool token account
   * @param tokenProgramId The program ID of the token program
   * @param swapProgramId The program ID of the token-swap program
   * @param tradeFeeNumerator Numerator of the fee ratio
   * @param tradeFeeDenominator Denominator of the fee ratio
   * @return Token object for the newly minted token, Public key of the account holding the total supply of new tokens
   */
  static async createTokenSwap(
    connection: Connection,
    payer: Account,
    tokenSwapAccount: Account,
    authority: PublicKey,
    tokenAccountA: PublicKey,
    tokenAccountB: PublicKey,
    poolToken: PublicKey,
    mintA: PublicKey,
    mintB: PublicKey,
    tokenFeeAccountA: PublicKey,
    tokenFeeAccountB: PublicKey,
    tokenAccountPool: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    tradeFeeNumerator: number,
    tradeFeeDenominator: number,
    confirmOptions?: ConfirmOptions,
  ): Promise<TokenSwap> {
    let transaction;
    const tokenSwap = new TokenSwap(
      connection,
      tokenSwapAccount.publicKey,
      swapProgramId,
      tokenProgramId,
      poolToken,
      tokenFeeAccountA,
      tokenFeeAccountB,
      authority,
      tokenAccountA,
      tokenAccountB,
      mintA,
      mintB,
      new Numberu64(tradeFeeNumerator),
      new Numberu64(tradeFeeDenominator),
      payer,
    );

    // Allocate memory for the account
    const balanceNeeded = await TokenSwap.getMinBalanceRentForExemptTokenSwap(
      connection,
    );
    transaction = new Transaction();
    transaction.add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: tokenSwapAccount.publicKey,
        lamports: balanceNeeded,
        space: TokenSwapLayout.span,
        programId: swapProgramId,
      }),
    );

    const instruction = TokenSwap.createInitSwapInstruction(
      tokenSwapAccount,
      authority,
      tokenAccountA,
      tokenAccountB,
      poolToken,
      tokenFeeAccountA,
      tokenFeeAccountB,
      tokenAccountPool,
      tokenProgramId,
      swapProgramId,
      tradeFeeNumerator,
      tradeFeeDenominator,
    );

    transaction.add(instruction);
    await sendAndConfirmTransaction(
      connection,
      transaction,
      [payer, tokenSwapAccount],
      confirmOptions,
    );

    return tokenSwap;
  }

  /**
   * Deposit tokens into the pool
   * @param userAccountA User account for token A
   * @param userAccountB User account for token B
   * @param poolAccount User account for pool token
   * @param userTransferAuthority Account delegated to transfer user's tokens
   * @param poolTokenAmount Amount of pool tokens to mint
   * @param maximumTokenA The maximum amount of token A to deposit
   * @param maximumTokenB The maximum amount of token B to deposit
   */
  async depositTokens(
    userAccountA: PublicKey,
    userAccountB: PublicKey,
    poolAccount: PublicKey,
    userTransferAuthority: Account,
    poolTokenAmount: number | Numberu64,
    maximumTokenA: number | Numberu64,
    maximumTokenB: number | Numberu64,
    confirmOptions?: ConfirmOptions,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      this.connection,
      new Transaction().add(
        TokenSwap.depositTokensInstruction(
          this.tokenSwap,
          this.authority,
          userTransferAuthority.publicKey,
          userAccountA,
          userAccountB,
          this.tokenAccountA,
          this.tokenAccountB,
          this.poolToken,
          poolAccount,
          this.swapProgramId,
          this.tokenProgramId,
          poolTokenAmount,
          maximumTokenA,
          maximumTokenB,
        ),
      ),
      [this.payer, userTransferAuthority],
      confirmOptions,
    );
  }

  static depositTokensInstruction(
    tokenSwap: PublicKey,
    authority: PublicKey,
    userTransferAuthority: PublicKey,
    sourceA: PublicKey,
    sourceB: PublicKey,
    intoA: PublicKey,
    intoB: PublicKey,
    poolToken: PublicKey,
    poolAccount: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    poolTokenAmount: number | Numberu64,
    maximumTokenA: number | Numberu64,
    maximumTokenB: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('poolTokenAmount'),
      Layout.uint64('maximumTokenA'),
      Layout.uint64('maximumTokenB'),
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 1, // Deposit instruction
        poolTokenAmount: new Numberu64(poolTokenAmount).toBuffer(),
        maximumTokenA: new Numberu64(maximumTokenA).toBuffer(),
        maximumTokenB: new Numberu64(maximumTokenB).toBuffer(),
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: userTransferAuthority, isSigner: true, isWritable: false},
      {pubkey: sourceA, isSigner: false, isWritable: true},
      {pubkey: sourceB, isSigner: false, isWritable: true},
      {pubkey: intoA, isSigner: false, isWritable: true},
      {pubkey: intoB, isSigner: false, isWritable: true},
      {pubkey: poolToken, isSigner: false, isWritable: true},
      {pubkey: poolAccount, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  /**
   * Withdraw tokens from the pool
   *
   * @param userAccountA User account for token A
   * @param userAccountB User account for token B
   * @param poolAccount User account for pool token
   * @param userTransferAuthority Account delegated to transfer user's tokens
   * @param poolTokenAmount Amount of pool tokens to burn
   * @param minimumTokenA The minimum amount of token A to withdraw
   * @param minimumTokenB The minimum amount of token B to withdraw
   */
  async withdrawTokens(
    userAccountA: PublicKey,
    userAccountB: PublicKey,
    poolAccount: PublicKey,
    userTransferAuthority: Account,
    poolTokenAmount: number | Numberu64,
    minimumTokenA: number | Numberu64,
    minimumTokenB: number | Numberu64,
    confirmOptions?: ConfirmOptions,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      this.connection,
      new Transaction().add(
        TokenSwap.withdrawTokensInstruction(
          this.tokenSwap,
          this.authority,
          userTransferAuthority.publicKey,
          this.poolToken,
          poolAccount,
          this.tokenAccountA,
          this.tokenAccountB,
          userAccountA,
          userAccountB,
          this.swapProgramId,
          this.tokenProgramId,
          poolTokenAmount,
          minimumTokenA,
          minimumTokenB,
        ),
      ),
      [this.payer, userTransferAuthority],
      confirmOptions,
    );
  }

  static withdrawTokensInstruction(
    tokenSwap: PublicKey,
    authority: PublicKey,
    userTransferAuthority: PublicKey,
    poolMint: PublicKey,
    sourcePoolAccount: PublicKey,
    fromA: PublicKey,
    fromB: PublicKey,
    userAccountA: PublicKey,
    userAccountB: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    poolTokenAmount: number | Numberu64,
    minimumTokenA: number | Numberu64,
    minimumTokenB: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('poolTokenAmount'),
      Layout.uint64('minimumTokenA'),
      Layout.uint64('minimumTokenB'),
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 2, // Withdraw instruction
        poolTokenAmount: new Numberu64(poolTokenAmount).toBuffer(),
        minimumTokenA: new Numberu64(minimumTokenA).toBuffer(),
        minimumTokenB: new Numberu64(minimumTokenB).toBuffer(),
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: userTransferAuthority, isSigner: true, isWritable: false},
      {pubkey: poolMint, isSigner: false, isWritable: true},
      {pubkey: sourcePoolAccount, isSigner: false, isWritable: true},
      {pubkey: fromA, isSigner: false, isWritable: true},
      {pubkey: fromB, isSigner: false, isWritable: true},
      {pubkey: userAccountA, isSigner: false, isWritable: true},
      {pubkey: userAccountB, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }

  /**
   * Swap token A for token B
   *
   * @param userSource User's source token account
   * @param poolSource Pool's source token account
   * @param poolDestination Pool's destination token account
   * @param userDestination User's destination token account
   * @param hostFeeAccount Host account to gather fees
   * @param userTransferAuthority Account delegated to transfer user's tokens
   * @param amountIn Amount to transfer from source account
   * @param minimumAmountOut Minimum amount of tokens the user will receive
   */
  async swap(
    userSource: PublicKey,
    poolSource: PublicKey,
    poolDestination: PublicKey,
    userDestination: PublicKey,
    userTransferAuthority: Account,
    feeAccount: PublicKey,
    amountIn: number | Numberu64,
    minimumAmountOut: number | Numberu64,
    confirmOptions?: ConfirmOptions,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      this.connection,
      new Transaction().add(
        TokenSwap.swapInstruction(
          this.tokenSwap,
          this.authority,
          userTransferAuthority.publicKey,
          userSource,
          poolSource,
          poolDestination,
          userDestination,
          feeAccount,
          this.swapProgramId,
          this.tokenProgramId,
          amountIn,
          minimumAmountOut,
        ),
      ),
      [this.payer, userTransferAuthority],
      confirmOptions,
    );
  }

  static swapInstruction(
    tokenSwap: PublicKey,
    authority: PublicKey,
    userTransferAuthority: PublicKey,
    userSource: PublicKey,
    poolSource: PublicKey,
    poolDestination: PublicKey,
    userDestination: PublicKey,
    feeAccount: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    amountIn: number | Numberu64,
    minimumAmountOut: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('amountIn'),
      Layout.uint64('minimumAmountOut'),
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 3, // Swap instruction
        amountIn: new Numberu64(amountIn).toBuffer(),
        minimumAmountOut: new Numberu64(minimumAmountOut).toBuffer(),
      },
      data,
    );

    const keys = [
      {pubkey: tokenSwap, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: userTransferAuthority, isSigner: true, isWritable: false},
      {pubkey: userSource, isSigner: false, isWritable: true},
      {pubkey: poolSource, isSigner: false, isWritable: true},
      {pubkey: poolDestination, isSigner: false, isWritable: true},
      {pubkey: userDestination, isSigner: false, isWritable: true},
      {pubkey: feeAccount, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: swapProgramId,
      data,
    });
  }
}
