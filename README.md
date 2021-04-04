# Konomi node

A substrate based node for DeFi innovation.

Currently there is a preliminary uniswap like AMM with functionalities of swap, add pool, add liquidity and remove liquidity. An auxiliary currency functionality is also provided to enable the AMM.

## Main Processes
### Supply Asset
When user supplies asset, if this asset is one of the allowed collateral, the user needs to choose if this asset is going to be used as collateral.

In Konomi, Internally, the system would update user supply interest, pool supply interest. To calculate the interest, there are many models that one can use. The current model is as follows:

When user supplies certain amount, the interest will only start calculation the next day, i.e. at UTC 00:00:00.  The interest rate is determined at time of deposit with the following equation:

```
UtilizationRatio = TotalBorrow / TotalSupply+TotalBorrow
SupplyingInterestRate = BorrowingInterestRate * UtilizationRatio
```

The choice of InitialInterestRate, UtilizationFactor are determined by the protocol.

To ensure the overall safety of the system, if the protocol deemed certain transaction invalid, it would reject the transaction. The system would reject the if the amount is more than the amount owned by the user.

Once all the checks are passed, the protocol would transfer the asset amount from user to the pool.

To calculate the user interest at the current time, we are performing as follows.

```json
# At the current time, the interest would be
Interest = InterestRate * TotalUserSupply * (CurrentTime - LastSupplyTime) / 360
```
### Withdraw Assets
To withdraw assets, the system would perform several checks to ensure the validity of the attempted transaction.

If the asset withdrawn is not one of the account's collateral, the amount withdrawn should not be more than that of amount supplied plus total interest earned. Otherwise, the transaction would be rejected.

If the asset is one of the account's collateral, the system would ensure liquidation process would not be triggered.

Liquidation would be triggered if the total asset supply in USD is fewer than a certain threshold of the total borrowed in USD. The detailed process is described as followed:

where in the above, `Borrowedi` refers to amount borrowed with interest for each asset, `ExchangeRatei` is the exchange rate of the i-th asset.

![equations/withdraw_0](equations/withdraw_0.png)

To calculate the amount left after the withdraw would be:

![equations/withdraw_1](equations/withdraw_1.png)

where in the above, `Suppliedi` refers to amount supply with interest for each asset, `Amount` refers to amount to withdraw for a specific asset. `ExchangeRatei` is the exchange rate of the i-th asset. `SafeFactori` is used to model certain reserve for liquidation, its value is between (0, 1).

If the following is reached, then the transaction would be rejected.

![equations/withdraw_2](equations/withdraw_2.png)

After that, the asset would be transferred from the pool to user account. And the amount withdraw would deducted from the asset pool.

### Borrow Asset

The user must have chosen the collateral and supply to the collateral before making borrow requests. To calculate the interest, there are many models that one can use. The current model is as follows:

When user supplies certain amount, the interest will start as of the current day.  The interest rate is determined at time of deposit with the following equation:

```
UtilizationRatio = TotalBorrow / TotalSupply
BowrrowingInterestRate = InitialInterestRate + UtilizationRatio * UtilizationFactor

Interest = InterestRate * TotalUserBorrow * (CurrentTime - LastBorrowTime) / 360
```

The choice of InitialInterestRate, UtilizationRatio are determined by the protocol. The current values are 2.5%, 20%.

To calculate the amount need for collateral is:

![equations/borrow_0](equations/borrow_0.png)

where in the above, `Borrowedi` refers to amount with interest borrowed for each asset, `ExchangeRatei` is the exchange rate of the i-th asset and `Amount` is the amount to borrow for that particular asset.

To calculate the current amount of collateral is:

![equations/borrow_1](equations/borrow_1.png)

where in the above, `Suppliedi` refers to amount with interest supply for each asset. `ExchangeRatei` is the exchange rate of the i-th asset. `SafeFactori` is used to model certain reserve for liquidation.

If the following is reached, then the transaction would be rejected.

![equations/borrow_2](equations/borrow_2.png)

Once all the checks are passed, the protocol would transfer the asset amount from user to the pool.

### Repay Asset

To repay assets, the system would update the interest as shown above. At the same time, if the amount repay is more than that the user owns, the transaction would be rejected.

After that, the asset would be transferred from user account to the pool.

### Liquidation
Liquidation will be triggered when the total supply is lower than the total borrowed, specifically in the following equation:

![equations/liquidation_0](equations/liquidation_0.png)

When the following condition is reached, the account would be marked as `Liquidable`. Arbitrageurs would be able liquidate the account.

![equations/liquidation_1](equations/liquidation_1.png)

The current value of `LiquidationThreshold` is 1. Konomi will provide an API for arbitrageurs to list the `HealthIndex` of all the account in ascending order. In each of the entry, arbitrageurs would be able to see the asset the account has borrowed.

### Arbitrage
Arbitrageurs would be able to supply to the borrowed asset pool of the liquidated account. For every single transaction, the arbitrageurs can only purchase up to `CloseFactor` of the assets. The current value is 1. If this `CloseFactor` is less than one and the total evaluation is less than 100 USD, the Arbitrageurs can purchase all the collateral.

**When arbitrageurs purchase from the liquidated account, the amount paid would be deducted from the liquidated account's asset borrowed and would go back to the pool of the asset. The equivalent amount of the collateral would be transferred from the liquidated account to the Konomi's account.**

To provide the context, assume the liquidated account used asset A as collateral and borrowed B asset. Arbitrageurs could provide B and get A in return. The collateral returned to the arbitrageurs are calculated as follows:

![equations/arbitrage](./equations/arbitrage.png)

where in the about, `Amount` refers to the amount supplied by arbitrageurs. `ExchangeRateB` is the exchange rate to USD of the borrowed asset. `DiscountFactor` is a number in the range of (0, 1), but now`DiscountFactor` is 0.95,it refers to an incentive for extra collateral returns to the arbitrageurs. `ExchangeRateA` is the exchange rate from USD to A.

## Pallets
- assets: asset for swap and lending

## Local Development

Follow these steps to prepare a local Substrate development environment:

### Simple Setup

Install all the required dependencies with a single command (be patient, this can take up to 30
minutes).

```bash
curl https://getsubstrate.io -sSf | bash -s -- --fast
```

### Build

Once the development environment is set up, build the node. This command will build the
[Wasm](https://substrate.dev/docs/en/knowledgebase/advanced/executor#wasm-execution) and
[native](https://substrate.dev/docs/en/knowledgebase/advanced/executor#native-execution) code:

```bash
cargo build --release
```

## Run

### Single Node Development Chain

Purge any existing dev chain state:

```bash
./target/release/konomi-node purge-chain --dev
```

Start a dev chain:

```bash
./target/release/konomi-node --dev
```