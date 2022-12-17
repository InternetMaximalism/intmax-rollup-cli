# intmax rollup CLI

ATTENTION: The Intmax testnet is still a pre-alpha version, so we will not commit data to L1 Network.

## Setup

```sh
git clone git@github.com:InternetMaximalism/intmax-rollup-cli.git
cd intmax-rollup-cli
cargo --version # 1.65.0-nightly
cargo build --release
alias intmax='./target/release/intmax-client'
intmax config aggregator-url https://prealpha.testnet.intmax.io/
```

## Getting Started

### Create your account

Initializing your wallet and delete your all accounts.

```sh
intmax account reset
```

Add default account (private key is selected randomly).

```sh
intmax account add --default
```

### Mint your token

Deposit your assets (the token contract address is the same with your address and the token id can be selected 0x00 - 0xff).

```sh
intmax deposit --amount 10 -i 0x00
```

### Send your assets

Merge your assets and Send your token to other accounts.

```sh
intmax tx send --amount 1 -i 0x00 --receiver-address 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d
```

### Reflect the results of the remittance

Merge your assets.

```sh
intmax tx merge
```

### Display your assets

Display your owned assets.

```sh
intmax assets
```

## How to Use

[Interface Guide](./docs/interface_guide.md)
