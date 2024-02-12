<div align="center">

  ![74763454](https://github.com/pucedoteth/intmax-rollup-cli/assets/119044801/2140fab8-a17b-4ae1-a95a-e8b464209e86)
    <h1>intmax</h1>
    <strong>The first stateless zkRollup</strong>
</div>
<br>

[![Twitter Follow](https://img.shields.io/twitter/follow/iintmaxIO?style=social)](https://twitter.com/intmaxIO)
[![Discord](https://img.shields.io/discord/984015101017346058?color=%235865F2&label=Discord&logo=discord&logoColor=%23fff)](https://discord.com/invite/N7kYGUPDEE)
[![Website Status](https://img.shields.io/website?url=https%3A%2F%2Fintmax.io)](https://intmax.io)

# intmax rollup CLI

ATTENTION: With the update of the project, this repository will be archived soon. Support and maintenance for this repository will be discontinued.

## Setup

For Ubuntu: [Install Guide (Ubuntu)](./docs/for_ubuntu.md)

### Clone this repository

```sh
# use SSH
git clone git@github.com:InternetMaximalism/intmax-rollup-cli.git -b staging
# or use HTTPS
git clone https://github.com/InternetMaximalism/intmax-rollup-cli.git -b staging
cd intmax-rollup-cli
git submodule update --init --recursive
```

### Build this CLI

```sh
cargo --version # 1.67.0-nightly
cargo run --release --bin intmax config aggregator-url https://alpha.testnet.intmax.io/
```

### Alias intmax command

```sh
alias intmax="$(pwd)/target/release/intmax"
intmax -V # intmax 2.2.1-alpha
```

## Update

If the CLI version has been updated, the following commands can be used to synchronize.

```sh
git checkout staging # only for users who have been using v1.0.2-alpha or earlier
git pull origin staging
cargo build --release
```

For more information on the release, check [here](https://github.com/InternetMaximalism/intmax-rollup-cli/releases).

## Getting Started

### Help

Display the help page.

```sh
intmax -h
```

### Create your account

Add default account (private key is selected randomly).

```sh
intmax account add --default --nickname alice
intmax account add --nickname bob
```

### Mint your token

Mint your token. The token address is the same with your address and the token id can be selected from 0x00 to 0xff.

```sh
intmax tx mint --amount 10 -i 0x00
```

### Send your assets

Merge your assets and Send your token to other accounts.

```sh
intmax tx send --amount 1 -i 0x00 --receiver-address bob
```

### Display your assets

Display your owned assets.

```sh
intmax account assets
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

## Interoperability

Please note that the following feature is currently in the **experimental** stage
and is intended to provide guidance for contract development.
Please be aware that at this time, we cannot guarantee the safe interoperability using this feature.

### Write private key

First, write the private key to the .env file.
In order to execute the commands that follow,
the account must have ETH deposited on Scroll alpha.

```sh
cp -n example.env .env
```

### Creating Another Account

To create another account with a nickname "carol", use the following command:

```sh
intmax account add --nickname carol
```

After executing this command, you will see a message that confirms that the account has been successfully created.
The message will contain a unique account address that is different every time the command is run.

```txt
new account added: 0xa27c8370eeddc4fe
```

### Making an Offer

To make an offer, you need tokens into your account.
If there is not a sufficient balance, use the following command to mint tokens:

```sh
intmax account set-default bob
intmax tx mint --amount 10 -i 0x00 -u bob
```

Once you have deposited tokens into your account,
you can create an offer by using the following command:

```sh
intmax io register --network scroll --maker-amount 1 --receiver-address carol --taker-token 0x0000000000000000000000000000000000000000 --taker-amount 1000000000000000 -u bob
```

Instead of sending 1 tokens of your own issue to the account created above,
make an offer requesting 0.001 ETH (= 10^15 wei) on the Scroll alpha testnet.
If `--taker-token` field is omitted, you will be prompted which token to use.
After executing this command, you will see a message that displays the offer ID.

```txt
start register()
end register()
offer_id: 0
WARNING: DO NOT interrupt execution of this program while a transaction is being sent.
start proving: user_tx_proof
prove: 4.614 sec
transaction hash is 0x8ae9fcd8825815dc21d9fad1841bfcc1375fb7727268f7dffc41952da47dc32d
broadcast transaction successfully
start proving: received_signature
prove: 0.028 sec
send received signature successfully
```

Make a note of this ID, as you will need it to activate the offer later.

### Switching to the Recipient's Address

Next, switch to the recipient's address.

```sh
intmax account set-default carol
```

### Activating the Offer

If the recipient accepts the offer, use the following command:

```sh
intmax io activate <offer-id> --network scroll
```

When the recipient accepts the offer,
ETH will be transferred on the Scroll alpha testnet at this time.
Therefore, it is necessary to deposit ETH in advance.

After activating the offer, use the following command to check your assets:

```sh
intmax account assets -u carol
```

You will see a message that displays the amount of tokens that you currently own.

```txt
User: carol
--------------------------------------------------------------------------------------
  Token Address | [bob]
  Token ID      | 0x00
  Amount        | 1
--------------------------------------------------------------------------------------
```

### Another Pattern

To create another account with a nickname "dave", use the following command:

```sh
intmax account add --nickname dave
```

Once you have deposited tokens into your account,
you can create an offer by using the following command:

```sh
intmax io lock --network scroll --maker-amount 1 --receiver <receiver-scroll-address> --receiver-address dave --taker-token 0x0000000000000000000000000000000000000000 --taker-amount 1000000000000000 -u carol
```

For example,

```sh
# Before executing, make sure that the address in the `--receiver` field is the one you own.
intmax io lock --network scroll --maker-amount 1 --receiver 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --receiver-address dave --taker-token 0x0000000000000000000000000000000000000000 --taker-amount 1000000000000000 -u carol
```

In this command, the `--receiver` field should contain the recipient's address on Scroll,
and the `--receiver-address` field should contain the recipient's address or nickname.
When you make an offer, ETH will be transferred on the Scroll alpha testnet in `<your-address>`.
Therefore, it is necessary to deposit ETH in advance.
After executing this command, you will see a message that displays the offer ID.

```txt
start lock()
end lock()
offer_id: 5
```

Next, switch to the recipient's address and mint your token.

```sh
intmax tx mint --amount 10 -i 0x00 -u dave
```

### Activating the Offer

To activate the offer, use the following command:

```sh
intmax io unlock <offer-id> --network scroll -u dave
```

After activating the offer, use the following command to check your assets:

```sh
intmax account assets -u carol
```

You will see a message that displays the amount of tokens that you currently own.

```txt
User: carol
--------------------------------------------------------------------------------------
  Token Address | dave
  Token ID      | 0x00
  Amount        | 1
--------------------------------------------------------------------------------------
```

<!--
## How to Use

[Interface Guide](./docs/interface_guide.md)
-->
