# dnt_perpetual_token

# 🏦 Delta-Neutral Perpetual Token ($DNT)
🚀 **Automated Risk-Neutral Trading for Perpetual Futures**

## 📌 Overview
**Delta-Neutral Perpetual Token ($DNT)** is a **Solana-based** protocol that **automates delta-neutral trading strategies**, enabling **risk-free arbitrage** and **stable returns** for traders. Users can **stake, borrow, earn rewards, and participate in governance**, while the protocol ensures market neutrality.

---

## 🔹 **Key Features**
### ✅ **Automated Delta-Neutral Trading**
- Ensures **zero net delta exposure** by **hedging long & short positions**.
- Traders **stake $DNT** to gain access to **automated delta-neutral pools**.

### ✅ **Dynamic Funding Rate Distribution**
- Rewards **$DNT stakers** with funding rate profits from perpetual futures markets.

### ✅ **Vault Profit Sharing**
- Distributes **arbitrage profits** back to **$DNT stakers**.

### ✅ **Liquidity Incentives for Market Makers**
- Rewards **high-volume liquidity providers** with additional **$DNT rebates**.

### ✅ **Flash Loan Prevention & Anti-Sybil Measures**
- Enforces **minimum staking duration** to prevent flash loan exploits.

### ✅ **Automated Liquidations & Risk Management**
- **Monitors positions** and **automatically liquidates** traders with excessive risk.

### ✅ **Multi-Collateral Support**
- Allows staking of **multiple assets**: `$DNT`, `SOL`, `USDC`, `USDT`.

### ✅ **Staked Voting & Decentralized Governance**
- $DNT holders **vote on risk parameters**, fee structures, and liquidity incentives.

---

## ⚙️ **Smart Contract(program) Architecture**
### 🏛 **Global State**
| Field | Type | Description |
|--------|------|------------|
| `bump` | `u8` | PDA bump for security |
| `total_staked` | `u64` | Total staked DNT in the system |
| `last_update` | `i64` | Timestamp of last reward update |
| `last_rebalance` | `i64` | Timestamp of last rebalancing |
| `allowed_delta_threshold` | `u64` | Governance-set risk threshold |

### 👤 **User Staking Account**
| Field | Type | Description |
|--------|------|------------|
| `amount` | `u64` | User’s staked DNT balance |
| `last_update` | `i64` | Timestamp of last staking action |

---
