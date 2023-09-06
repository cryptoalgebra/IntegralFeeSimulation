# Algebra Integral volatility fee simulation tool

A tool for conducting large-scale simulations of Algebra Integral volatility oracle nad adaptive fee behavior using historical data.

Forked from https://github.com/primitivefinance/arbiter-template

## Usage

1. Clone this repository

```
git clone https://github.com/cryptoalgebra/IntegralFeeSimulation.git
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
