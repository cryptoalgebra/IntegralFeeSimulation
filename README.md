# Algebra Integral volatility fee simulation tool

A tool for conducting large-scale simulations of Algebra Integral volatility oracle nad adaptive fee behavior using historical data.

Forked from https://github.com/primitivefinance/arbiter-template

## Input data format

Tool expects `swaps.json` file in `input` directory. File must contain one array with `Swap` event structs, sorted by time and event order in tx/block.

Each even must contain following fields:
- `timestamp` - in seconds, since start of UNIX time (same as `block.timestamp` for almost every EVM-compatible chain)
- `tick` - tick in the pool after the swap (This value is present in the `Swap` event, which is emitted by the pool after the swap)

The easiest way to get the required data is to download swap events for a UniswapV3 or Algebra pool and then add timestamps to them.

## Output data format

Tool will generate `result.json` file in `output` directory. File will contain one array with structures, sorted by timestamps.

Structures will contain following fields:
- `timestamp` - in seconds, since start of UNIX time (same as `block.timestamp` for almost every EVM-compatible chain)
- `tickAverage` - average tick for 24h window
- `fee` - adaptive fee value for this timestamp (with default configuration)
- `volatilityAverage` - average volatility for 24h window
- `tick` - the tick at the start of this block (before any swaps)

The obtained data, in particular, about the average volatility, can be conveniently used to compare adaptive fee values ​​for different adaptive fee configurations.

## Usage

1. Clone this repository

```
git clone https://github.com/cryptoalgebra/IntegralFeeSimulation.git --recursive
cd IntegralFeeSimulation
```

2. Install foundry

```
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

3. Install forge libraries

```
forge install
```

4. Generate bindings

```
forge bind --revert-strings debug -b src/bindings/ --module --overwrite
```

5. Put input data in `input/swaps.json`


6. Run the project

```
cargo run
```

7. Output data will be in `output/result.json`
