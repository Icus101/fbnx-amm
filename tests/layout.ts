import * as BufferLayout from "buffer-layout";
import * as anchor from "@project-serum/anchor";
import { TypeDef } from "@project-serum/anchor/dist/cjs/program/namespace/types";

export const CurveType = Object.freeze({
    ConstantProduct : 0,
    ConstantPrice : 1
  })

  const SWAP_PROGRAM_OWNER_FEE_ADDRESS =
  process.env.SWAP_PROGRAM_OWNER_FEE_ADDRESS;

 const TRADING_FEE_NUMERATOR = 25;
  const TRADING_FEE_DENOMINATOR = 10000;
  const OWNER_TRADING_FEE_NUMERATOR = 5;
  const OWNER_TRADING_FEE_DENOMINATOR = 10000;
  const OWNER_WITHDRAW_FEE_NUMERATOR = SWAP_PROGRAM_OWNER_FEE_ADDRESS ? 0 : 1;
  const OWNER_WITHDRAW_FEE_DENOMINATOR = SWAP_PROGRAM_OWNER_FEE_ADDRESS ? 0 : 6;
  const HOST_FEE_NUMERATOR = 20;
  const HOST_FEE_DENOMINATOR = 100;


  export const commandDataLayout = BufferLayout.struct([
    BufferLayout.nu64("tradeFeeNumerator"),
    BufferLayout.nu64("tradeFeeDenominator"),
    BufferLayout.nu64("ownerTradeFeeNumerator"),
    BufferLayout.nu64("ownerTradeFeeDenominator"),
    BufferLayout.nu64("ownerWithdrawFeeNumerator"),
    BufferLayout.nu64("ownerWithdrawFeeDenominator"),
    BufferLayout.nu64("hostFeeNumerator"),
    BufferLayout.nu64("hostFeeDenominator"),
    BufferLayout.u8("curveType"),
    BufferLayout.nu64("curveParameters"),
    // BufferLayout.blob(32, 'curveParameters'),
  ]);

  let data = Buffer.alloc(1024);
    export const encodeLength = commandDataLayout.encode(
      {
        tradeFeeNumerator: TRADING_FEE_NUMERATOR,
        tradeFeeDenominator: TRADING_FEE_DENOMINATOR,
        ownerTradeFeeNumerator: OWNER_TRADING_FEE_NUMERATOR,
        ownerTradeFeeDenominator: OWNER_TRADING_FEE_DENOMINATOR,
        ownerWithdrawFeeNumerator: OWNER_WITHDRAW_FEE_NUMERATOR,
        ownerWithdrawFeeDenominator: OWNER_WITHDRAW_FEE_DENOMINATOR,
        hostFeeNumerator: HOST_FEE_NUMERATOR,
        hostFeeDenominator: HOST_FEE_DENOMINATOR,
        curveType: CurveType.ConstantProduct,
        curveParameters: 0,
      },
      data
    );
    data = data.slice(0, encodeLength);

    export const fees_input: TypeDef<
    {
      name: "FeesInput";
      type: {
        kind: "struct";
        fields: [
          {
            name: "tradeFeeNumerator";
            type: "u64";
          },
          {
            name: "tradeFeeDenominator";
            type: "u64";
          },
          {
            name: "ownerTradeFeeNumerator";
            type: "u64";
          },
          {
            name: "ownerTradeFeeDenominator";
            type: "u64";
          },
          {
            name: "ownerWithdrawFeeNumerator";
            type: "u64";
          },
          {
            name: "ownerWithdrawFeeDenominator";
            type: "u64";
          },
          {
            name: "hostFeeNumerator";
            type: "u64";
          },
          {
            name: "hostFeeDenominator";
            type: "u64";
          }
        ];
      };
    },
    Record<string, number>
  > = {
    tradeFeeNumerator: new anchor.BN(TRADING_FEE_NUMERATOR),
    tradeFeeDenominator: new anchor.BN(TRADING_FEE_DENOMINATOR),
    ownerTradeFeeNumerator: new anchor.BN(OWNER_TRADING_FEE_NUMERATOR),
    ownerTradeFeeDenominator: new anchor.BN(OWNER_TRADING_FEE_DENOMINATOR),
    ownerWithdrawFeeNumerator: new anchor.BN(OWNER_WITHDRAW_FEE_NUMERATOR),
    ownerWithdrawFeeDenominator: new anchor.BN(
      OWNER_WITHDRAW_FEE_DENOMINATOR
    ),
    hostFeeNumerator: new anchor.BN(HOST_FEE_NUMERATOR),
    hostFeeDenominator: new anchor.BN(HOST_FEE_DENOMINATOR),
  };
  
  export const curve_input: TypeDef<
  {
    name: "CurveInput";
    type: {
      kind: "struct";
      fields: [
        {
          name: "curveType";
          type: "u8";
        },
        {
          name: "curveParameters";
          type: "u64";
        }
      ];
    };
  },
  Record<string, number >
> = {
  curveType: CurveType.ConstantProduct,
  curveParameters: new anchor.BN(0),
};

