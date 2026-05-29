# NFT Staking — Anchor + MPL Core

A Solana staking program built with Anchor and Metaplex Core (MPL Core). Users stake NFTs from a registered collection to earn reward tokens over time. Rewards can be claimed independently without unstaking, and the collection tracks how many NFTs are currently staked via an on-chain Attributes plugin.

## Assignment Challenges

**Challenge 1 — Separate claim rewards**
`claim_rewards` is its own instruction. It mints reward tokens and resets the reward clock (`staked_at`) without touching the NFT. Users can claim at any time and then immediately unstake — both paths are fully supported.

**Challenge 2 — Collection Attributes plugin**
The collection NFT holds an `Attributes` plugin with a `staked_count` key. It increments on every `stake` call and decrements on every `unstake` call, giving an accurate on-chain count of currently staked NFTs from this collection.

## Program Instructions

| Instruction | Description |
|---|---|
| `initialize` | Creates the global `StakeConfig` PDA and the caller's `UserAccount` PDA. Sets `rewards_per_day = 10` and initializes the reward mint with the config PDA as mint authority. |
| `mint_asset` | Mints a new MPL Core NFT into the staking collection via CPI. |
| `stake` | Adds a `FreezeDelegate` plugin to the NFT (frozen = true), creates a `StakeAccount` PDA, increments `user_account.amount_staked`, and increments the collection's `staked_count` attribute. |
| `claim_rewards` | Calculates elapsed days since `staked_at`, mints reward tokens proportional to time staked, accumulates points on `user_account`, and resets `staked_at` to now. The NFT stays frozen. |
| `unstake` | Auto-claims any pending rewards, thaws the NFT by setting `FreezeDelegate { frozen: false }`, removes the plugin, closes the `StakeAccount`, decrements `user_account.amount_staked`, and decrements the collection's `staked_count`. |

## On-Chain Accounts

| Account | Seeds | Description |
|---|---|---|
| `StakeConfig` | `["config"]` | Global config — rewards rate, freeze period, collection, rewards mint |
| `UserAccount` | `["user", wallet]` | Per-user — accumulated points and staked NFT count |
| `StakeAccount` | `["stake", mint, config]` | Per-NFT — owner, mint, `staked_at` timestamp |

## Reward Formula

```
rewards = elapsed_days * rewards_per_day * 10^decimals
```

- `rewards_per_day` = 10 (set at initialize)
- `decimals` = 6
- Reward after 1 day = 10,000,000 base units = 10 tokens

## Project Structure

```
programs/anchor-nft-stake/src/
├── lib.rs
├── errors.rs
└── state/
    ├── mod.rs
    ├── config.rs                  ← StakeConfig, UserAccount, StakeAccount
└── instructions/
    ├── mod.rs
    ├── initialize.rs
    ├── mint_asset.rs
    ├── stake.rs
    ├── unstake.rs
    └── claim_rewards.rs
tests/
└── anchor-nft-stake.ts       ← 8 integration tests
```

## Dependencies

```toml
anchor-lang = "0.31.0"
anchor-spl  = "0.31.0"
mpl-core    = "0.11.1"
```

## Setup & Build

```bash
# Install dependencies
yarn install

# Build the program
anchor build
```

## Running Tests

```bash
surfpool start
```
```bash
anchor test --skip-local-validator --skip-deploy
```

Expected output:
```
✔ initializes staking config and user account with correct default values
✔ rejects a second initialize call on the same config PDA
✔ stakes an NFT, freezes it via FreezeDelegate, and records staked_at timestamp
✔ rejects staking an already-staked NFT because stake PDA already exists
✔ rejects claim_rewards when less than 1 full day has elapsed since staking
✔ unstakes NFT, thaws it, closes stake account, and decrements user staked count
✔ mints a new collection NFT via the mint_asset program instruction
✔ allows the same NFT to be re-staked after a full unstake (round-trip)

8 passing
```

## Screenshot

<img width="2572" height="1052" alt="image" src="https://github.com/user-attachments/assets/79c2b457-7d09-4e81-9832-3ed2aa4a6579" />


