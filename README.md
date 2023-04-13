# intmax rollup CLI

ATTENTION: The Intmax testnet is still a pre-alpha version, so we will not commit data to L1 Network.

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
intmax -V # intmax 2.0.2-alpha
```

## Update

If the CLI version has been updated, the following commands can be used to synchronize.

```sh
git checkout staging # only for users who have been using v1.0.2-alpha or earlier
git pull origin staging
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
intmax tx mint --amount 10 -i 0x00
```

Once you have deposited tokens into your account,
you can create an offer by using the following command:

```sh
intmax io register --network scroll --maker-amount 1 --receiver-address carol --taker-amount 1000000000000000
```

Instead of sending 10 tokens of your own issue to the account created here,
make an offer requesting 0.001 ETH (= 10^15 wei) on the Scroll alpha testnet.
The `--receiver-address` field in this command should contain the recipient's address as it appears on your screen.
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
intmax account assets
```

You will see a message that displays the amount of tokens that you currently own.

```txt
User: carol
--------------------------------------------------------------------------------------
  Token Address | [alice]
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
intmax account set-default carol
intmax io lock --network scroll --maker-amount 1 --receiver <your-address> --receiver-address dave --taker-amount 1000000000000000
```

In this command, the `--receiver` field should contain the recipient's address in Ethereum,
and the `--receiver-address` field should contain the recipient's address or nickname.
When you make an offer,
ETH will be transferred on the Scroll alpha testnet at this time.
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
