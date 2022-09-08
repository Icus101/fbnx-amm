import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { FbnxAmm } from "../target/types/fbnx_amm";

describe("fbnx-amm", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.FbnxAmm as Program<FbnxAmm>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
