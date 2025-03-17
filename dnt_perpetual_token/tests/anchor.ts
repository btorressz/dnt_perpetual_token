import * as anchor from "@coral-xyz/anchor";
import BN from "bn.js";
import assert from "assert";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import type { DntPerpetualToken } from "../target/types/dnt_perpetual_token";

describe("dnt_perpetual_token", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.getProvider();
  const program = anchor.workspace.DntPerpetualToken as anchor.Program<DntPerpetualToken>;

  it("Initializes the state", async () => {
    // Derive the state PDA using seeds: "state" and payer's public key.
    const [statePDA, bump] = await PublicKey.findProgramAddress(
      [Buffer.from("state"), provider.publicKey.toBuffer()],
      program.programId
    );

    // Call initialize with the proper accounts.
    const tx = await program.methods.initialize()
      .accounts({
        state: statePDA,
        payer: provider.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("Initialized state with tx:", tx);

    // Fetch the state account and assert that initialization is correct.
    const stateAccount = await program.account.state.fetch(statePDA);
    assert.ok(new BN(stateAccount.totalStaked).eq(new BN(0)));
    assert.strictEqual(stateAccount.allowedDeltaThreshold.toNumber(), 100);
  });
});
