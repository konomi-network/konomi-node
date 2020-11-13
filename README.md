# Konomi node

A substrate based node for DeFi innovation.

Currently there is a preliminary uniswap like AMM with functionalities of swap, add pool, add liquidity and remove liquidity. An auxiliary currency functionality is also provided to enable the AMM.

## pallets
- assets: asset for swap and lending
- swap: swap, add liquidity and remove liquidity functionalities

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
