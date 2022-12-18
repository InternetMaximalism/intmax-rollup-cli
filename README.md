# intmax rollup CLI

ATTENTION: The Intmax testnet is still a pre-alpha version, so we will not commit data to L1 Network.

## Setup

```sh
git clone git@github.com:InternetMaximalism/intmax-rollup-cli.git
cd intmax-rollup-cli
cargo --version # 1.65.0-nightly
cargo build --release
alias intmax='./target/release/intmax'
intmax config aggregator-url https://prealpha.testnet.intmax.io/
```

## Getting Started

### Help

Display the help page.

```sh
intmax -h
```

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

Mint your token. The token address is the same with your address and the token id can be selected from 0x00 to 0xff.

```sh
intmax deposit --amount 10 -i 0x00
```

### Send your assets

Merge your assets and Send your token to other accounts.

```sh
intmax tx send --amount 1 -i 0x00 --receiver-address 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d
```

### Display your assets

Display your owned assets.

```sh
intmax assets
```

### Bulk-mint

You can issue new token according to the contents of the file. Up to 16 tokens can be sent together in the testnet.

```sh
intmax tx bulk-mint -f ./tests/airdrop/example.csv
```

### Bulk-transfer

You can transfer owned tokens according to the contents of the file. You can send several tokens together in one transaciton.
The number of this aggregation is limited to 8 tokens in the testnet, and will be set to maximum 1024 in the mainnet.

```sh
intmax tx bulk-mint -f ./tests/airdrop/example2.csv
intmax tx bulk-transfer -f ./tests/airdrop/example3.csv
```

## How to Use

[Interface Guide](./docs/interface_guide.md)
