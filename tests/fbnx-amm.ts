import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { FbnxAmm } from "../target/types/fbnx_amm";
import * as BufferLayout from "buffer-layout";
import { PublicKey, Connection, Commitment } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID,ASSOCIATED_TOKEN_PROGRAM_ID, createMint,getMint,getAccount} from "@solana/spl-token";
import * as token from "@solana/spl-token"
import { assert } from "chai";
import { TypeDef } from "@project-serum/anchor/dist/cjs/program/namespace/types";
import NodeWallet from "@project-serum/anchor/dist/cjs/nodewallet";
import {encodeLength,curve_input,fees_input} from "../tests/layout"


// const CurveType = Object.freeze({
//   ConstantProduct : 0,
//   ConstantPrice : 1
// })

describe("fbnx-amm", () => {
  const commitment: Commitment = "processed";
  const connection = new Connection("https://rpc-mainnet-fork.dappio.xyz", {
    commitment,
    wsEndpoint: "wss://rpc-mainnet-fork.dappio.xyz/ws",
  });
  // Configure the client to use the local cluster.

  
  anchor.setProvider(anchor.AnchorProvider.env());

  const wallet = NodeWallet.local();
  const program = anchor.workspace.FbnxAmm as Program<FbnxAmm>;
  
  const provider = new anchor.AnchorProvider(connection, wallet, {
    preflightCommitment: 'processed',
});

  const SWAP_PROGRAM_OWNER_FEE_ADDRESS =
    process.env.SWAP_PROGRAM_OWNER_FEE_ADDRESS;

    let mintA  ;
    let mintB;
    let tokenPool
    let tokenAccountA: PublicKey;
    let tokenAccountB: PublicKey;
    const payer = anchor.web3.Keypair.generate();
  const owner = anchor.web3.Keypair.generate();
  let authority: PublicKey;
  let bumpSeed: number;
  let tokenAccountPool: PublicKey;
  let feeAccount: PublicKey;


  const TRADING_FEE_NUMERATOR = 25;
  const TRADING_FEE_DENOMINATOR = 10000;
  const OWNER_TRADING_FEE_NUMERATOR = 5;
  const OWNER_TRADING_FEE_DENOMINATOR = 10000;
  const OWNER_WITHDRAW_FEE_NUMERATOR = SWAP_PROGRAM_OWNER_FEE_ADDRESS ? 0 : 1;
  const OWNER_WITHDRAW_FEE_DENOMINATOR = SWAP_PROGRAM_OWNER_FEE_ADDRESS ? 0 : 6;
  const HOST_FEE_NUMERATOR = 20;
  const HOST_FEE_DENOMINATOR = 100;


  // const commandDataLayout = BufferLayout.struct([
  //   BufferLayout.nu64("tradeFeeNumerator"),
  //   BufferLayout.nu64("tradeFeeDenominator"),
  //   BufferLayout.nu64("ownerTradeFeeNumerator"),
  //   BufferLayout.nu64("ownerTradeFeeDenominator"),
  //   BufferLayout.nu64("ownerWithdrawFeeNumerator"),
  //   BufferLayout.nu64("ownerWithdrawFeeDenominator"),
  //   BufferLayout.nu64("hostFeeNumerator"),
  //   BufferLayout.nu64("hostFeeDenominator"),
  //   BufferLayout.u8("curveType"),
  //   BufferLayout.nu64("curveParameters"),
  //   // BufferLayout.blob(32, 'curveParameters'),
  // ]);

  let data = Buffer.alloc(1024);
    // const encodeLength = commandDataLayout.encode(
    //   {
    //     tradeFeeNumerator: TRADING_FEE_NUMERATOR,
    //     tradeFeeDenominator: TRADING_FEE_DENOMINATOR,
    //     ownerTradeFeeNumerator: OWNER_TRADING_FEE_NUMERATOR,
    //     ownerTradeFeeDenominator: OWNER_TRADING_FEE_DENOMINATOR,
    //     ownerWithdrawFeeNumerator: OWNER_WITHDRAW_FEE_NUMERATOR,
    //     ownerWithdrawFeeDenominator: OWNER_WITHDRAW_FEE_DENOMINATOR,
    //     hostFeeNumerator: HOST_FEE_NUMERATOR,
    //     hostFeeDenominator: HOST_FEE_DENOMINATOR,
    //     curveType: CurveType.ConstantProduct,
    //     curveParameters: 0,
    //   },
    //   data
    // );
    data = data.slice(0, encodeLength);

  //   const fees_input: TypeDef<
  //   {
  //     name: "FeesInput";
  //     type: {
  //       kind: "struct";
  //       fields: [
  //         {
  //           name: "tradeFeeNumerator";
  //           type: "u64";
  //         },
  //         {
  //           name: "tradeFeeDenominator";
  //           type: "u64";
  //         },
  //         {
  //           name: "ownerTradeFeeNumerator";
  //           type: "u64";
  //         },
  //         {
  //           name: "ownerTradeFeeDenominator";
  //           type: "u64";
  //         },
  //         {
  //           name: "ownerWithdrawFeeNumerator";
  //           type: "u64";
  //         },
  //         {
  //           name: "ownerWithdrawFeeDenominator";
  //           type: "u64";
  //         },
  //         {
  //           name: "hostFeeNumerator";
  //           type: "u64";
  //         },
  //         {
  //           name: "hostFeeDenominator";
  //           type: "u64";
  //         }
  //       ];
  //     };
  //   },
  //   Record<string, number>
  // > = {
  //   tradeFeeNumerator: new anchor.BN(TRADING_FEE_NUMERATOR),
  //   tradeFeeDenominator: new anchor.BN(TRADING_FEE_DENOMINATOR),
  //   ownerTradeFeeNumerator: new anchor.BN(OWNER_TRADING_FEE_NUMERATOR),
  //   ownerTradeFeeDenominator: new anchor.BN(OWNER_TRADING_FEE_DENOMINATOR),
  //   ownerWithdrawFeeNumerator: new anchor.BN(OWNER_WITHDRAW_FEE_NUMERATOR),
  //   ownerWithdrawFeeDenominator: new anchor.BN(
  //     OWNER_WITHDRAW_FEE_DENOMINATOR
  //   ),
  //   hostFeeNumerator: new anchor.BN(HOST_FEE_NUMERATOR),
  //   hostFeeDenominator: new anchor.BN(HOST_FEE_DENOMINATOR),
  // };

  // Initial amount in each swap token
  let currentSwapTokenA = 1000000;
  let currentSwapTokenB = 1000000;

  // const curve_input: TypeDef<
  //     {
  //       name: "CurveInput";
  //       type: {
  //         kind: "struct";
  //         fields: [
  //           {
  //             name: "curveType";
  //             type: "u8";
  //           },
  //           {
  //             name: "curveParameters";
  //             type: "u64";
  //           }
  //         ];
  //       };
  //     },
  //     Record<string, number | u64>
  //   > = {
  //     curveType: CurveType.ConstantProduct,
  //     curveParameters: new anchor.BN(0),
  //   };

  it("Is initialized!", async () => {

    tokenPool = await token.createMint(
      provider.connection,
      payer,
      authority,
      null,
      2,
    );

    
    const ammAccount = anchor.web3.Keypair.generate();

    [authority, bumpSeed] = await PublicKey.findProgramAddress(
      [ammAccount.publicKey.toBuffer()],
      program.programId
    );

     // creating pool account
     tokenAccountPool = await tokenPool.createAccount(owner.publicKey);
     const ownerKey =
       SWAP_PROGRAM_OWNER_FEE_ADDRESS || owner.publicKey.toString();
     feeAccount = await tokenPool.createAccount(new PublicKey(ownerKey));
 

    // creating token A
    mintA = await createMint(
      provider.connection,
      payer,
      owner.publicKey,
      null,
      2,
    );

    // creating token A account
    tokenAccountA = await mintA.createAccount(authority);
    // minting token A to swap
    await mintA.mintTo(tokenAccountA, owner, [], currentSwapTokenA);

    tokenPool = await createMint(
      provider.connection,
      payer,
      authority,
      null,
      2,
    );

    // creating token B
    mintB = await createMint(
      provider.connection,
      payer,
      owner.publicKey,
      null,
      2,
    );

    // creating token B account
    tokenAccountB = await mintB.createAccount(authority);
    // minting token B to swap
    await mintB.mintTo(tokenAccountB, owner, [], currentSwapTokenB);

    const poolMintInfo = await tokenPool.getMintInfo()


    // Add your test here.
    const tx = await program.rpc.initPool(fees_input,curve_input,{
      accounts : {
        poolAuthority: authority,
        amm : ammAccount.publicKey,
        poolMint : tokenPool ,
        vault0 : tokenAccountA ,
        vault1 : tokenAccountB,
        feeAccount : feeAccount,
        destination : ,
        payer : payer.publicKey,
        mint0 : mintA ,
        mint1 : mintB,
        tokenProgram : TOKEN_PROGRAM_ID,
        associatedTokenProgram : ASSOCIATED_TOKEN_PROGRAM_ID,
        rent :anchor.web3.SYSVAR_RENT_PUBKEY ,
        systemProgram :anchor.web3.SystemProgram.programId ,

      }
    });
    console.log("Your transaction signature", tx);
  });

  it("DepositAllTokenTypes", async () => {
    const poolMintInfo = await token.getMintInfo();
    const supply = (poolMintInfo.supply as anchor.BN).toNumber();
    const swapTokenA = await mintA.getAccountInfo(tokenAccountA);
    const tokenAAmount = Math.floor(
      ((swapTokenA.amount as anchor.BN).toNumber() * POOL_TOKEN_AMOUNT) / supply
    );
    const swapTokenB = await mintB.getAccountInfo(tokenAccountB);
    const tokenBAmount = Math.floor(
      ((swapTokenB.amount as anchor.BN).toNumber() * POOL_TOKEN_AMOUNT) / supply
    );

    const userTransferAuthority = anchor.web3.Keypair.generate();
    // Creating depositor token a account
    const userAccountA = await mintA.createAccount(owner.publicKey);
    await mintA.mintTo(userAccountA, owner, [], tokenAAmount);
    await mintA.approve(
      userAccountA,
      userTransferAuthority.publicKey,
      owner,
      [],
      tokenAAmount
    );
    // Creating depositor token b account
    const userAccountB = await mintB.createAccount(owner.publicKey);
    await mintB.mintTo(userAccountB, owner, [], tokenBAmount);
    await mintB.approve(
      userAccountB,
      userTransferAuthority.publicKey,
      owner,
      [],
      tokenBAmount
    );
    // Creating depositor pool token account
    const newAccountPool = await tokenPool.createAccount(owner.publicKey);

    // Depositing into swap
    await program.rpc.depositAll(
      new anchor.BN(POOL_TOKEN_AMOUNT),
      new anchor.BN(tokenAAmount),
      new anchor.BN(tokenBAmount),
      {
        accounts: {
          authority: authority,
          amm: ammAccount.publicKey,
          userTransferAuthorityInfo: userTransferAuthority.publicKey,
          sourceAInfo: userAccountA,
          sourceBInfo: userAccountB,
          tokenA: tokenAccountA,
          tokenB: tokenAccountB,
          poolMint: tokenPool.publicKey,
          destination: newAccountPool,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [userTransferAuthority],
      }
    );

});
