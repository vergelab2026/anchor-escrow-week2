# Escrow (Anchor / Solana)

Week 2 assignment: a token escrow program built with Anchor. A maker locks token A, and a taker can swap it for token B. The maker can cancel (refund) or change the requested amount (update). Tested with LiteSVM.

## Overview

- The maker deposits `amount` of `mint_a` into a vault that is owned by the escrow PDA.
- The maker sets `receive`, the amount of `mint_b` they want in return.
- The taker pays `receive` of `mint_b` to the maker and receives the vault's `mint_a`. Both the vault and the escrow are then closed.

## Accounts and PDAs

| Account | Seeds | Purpose |
|---|---|---|
| `escrow` | `["escrow", maker, seed]` | Stores maker, mint_a, mint_b, receive, seed, bump |
| `vault` | associated token account of `escrow` for `mint_a` | Holds the maker's deposited `mint_a` |

`seed` lets the same maker create several escrows.

## Instructions

- **`make(seed, receive, amount)`**: Creates the escrow and the vault, and moves `amount` of `mint_a` from the maker to the vault. `amount` must be greater than 0.
- **`take()`**: The taker sends `receive` of `mint_b` to the maker. The vault's `mint_a` goes to the taker. The vault and the escrow are closed, and their rent goes to the maker.
- **`refund()`**: Only the maker can call this. The vault's `mint_a` returns to the maker, and the vault and escrow are closed.
- **`update(receive)`**: Only the maker can call this. Changes the requested `receive` amount.

## Errors

- `InvalidAmount`: The amount is zero.
- `NotMaker`: Defined for maker-only actions. The `has_one = maker` constraint currently rejects non-makers.

## Tests

`tests/escrow.rs` runs in LiteSVM (a local simulated Solana environment) and covers:

1. `make_then_take_swaps_tokens_and_closes_accounts`: balances after the swap, and the vault and escrow are closed.
2. `make_then_refund_returns_tokens_to_maker`: the maker gets all of `mint_a` back, and the vault and escrow are closed.
3. `update_changes_the_requested_amount`: the stored `receive` value changes.
4. `make_with_zero_amount_fails`: a zero amount is rejected.
5. `only_the_maker_can_refund`: a taker cannot call `refund` on the maker's escrow.

`src/lib.rs` also has a unit test (`test_id`) that checks the program ID.

## How to run

```bash
anchor build
anchor test
```

Current result: all tests pass (`test result: ok`).

## Environment

- anchor-cli 1.1.2
- solana-cli 3.1.10
- Rust 1.89.0 (pinned in `rust-toolchain.toml`)
- anchor-lang and anchor-spl are pinned to 1.1.2 in `Cargo.toml`
- Windows 11 with WSL2 (Ubuntu)

## Notes

- `init_if_needed` requires the `init-if-needed` feature of `anchor-lang`, which is enabled in `Cargo.toml`.
- Large account structs are wrapped in `Box` to stay under Solana's 4 KB stack limit.
