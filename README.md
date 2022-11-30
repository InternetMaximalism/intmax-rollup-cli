# intmax rollup CLI

## Setup

```sh
git clone git@github.com:InternetMaximalism/intmax-rollup-cli.git
cd intmax-rollup-cli
rustup override set nightly
cargo --version # >= 1.65.0-nightly
cargo build --release
alias intmax='./target/release/intmax-client'
intmax config aggregator-url https://prealpha.testnet.intmax.io/
```

## How to use

```sh
intmax account reset # Initializing your wallet and delete your all accounts
intmax account add --default # Add default account (private key is selected randomly)
intmax account add --private-key 0x01 # Add account with address: 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d
```

```sh
intmax deposit --amount 10 --contract-address 0x01 -i 0x00 # Deposit your assets
intmax block propose # aggregator's operation
intmax block approve # aggregator's operation
```

```sh
intmax tx send --amount 1 --contract-address 0x01 -i 0x00 --receiver-address 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d # Merge your assets and Send asset to other accounts
intmax block propose # aggregator's operation
intmax block sign # Sign to proposed block (called received signature)
intmax block approve # aggregator's operation
```

```sh
intmax tx merge # Merge your assets
intmax block propose # aggregator's operation
intmax block sign
intmax block approve # aggregator's operation
```

```sh
intmax assets # Display your owned assets (excluding pre-merge)
```

```sh
intmax account set-default 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d # Change default account
```

```sh
intmax tx merge
intmax block propose # aggregator's operation
intmax block sign
intmax block approve # aggregator's operation
intmax assets
```
