import {
  createAccountAndSwapAtomic,
  createTokenSwap,
  swap,
  depositTokens,
  withdrawTokens,
} from './token-swap-test';

async function main() {
  // These test cases are designed to run sequentially and in the following order
  console.log(
    'Run test: createTokenSwap (constant product, used further in tests)',
  );
  await createTokenSwap();
  console.log('Run test: deposit all token types');
  await depositTokens();
  console.log('Run test: withdraw all token types');
  await withdrawTokens();
  console.log('Run test: swap');
  await swap();
  console.log('Run test: create account, approve, swap all at once');
  await createAccountAndSwapAtomic();
  console.log('Success\n');
}

main()
  .catch(err => {
    console.error(err);
    process.exit(-1);
  })
  .then(() => process.exit());
