# Interface Guide

This page describes how to use it.

## Create account

Creates a new account. If the default flag is given, it is used if user-address is omitted in commands that require it.
A hex string with 0x-prefix of less than or equal to 32 bytes can be given as private key.

```
intmax account add [--default] [--private-key <private-key>]
```

### Example

Add default account (private key is selected randomly)

```sh
intmax account add --default
```

Add default account with private key

```sh
intmax account add --default --private-key 0x1234
```

#### success response

```
new account added: 0x30cc462351a42c905ad6e846d939e1afb8677735480459975965dc8bb23570ad
set above account as default
```

<!-- #### failure response

```
Error: designated address was already used
``` -->

## Display accounts

You can check the account list.

```sh
intmax account list
```

### Example

#### success response

```
0x30cc462351a42c905ad6e846d939e1afb8677735480459975965dc8bb23570ad (default)
0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d
```

## Change default account

You can change the account used as default.

```
intmax account set-default [user-address]
```

### Example

```sh
intmax account set-default 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d
```

#### success response

```
set default account: 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d
```

<!-- #### failure response

```
Error: given account does not exist in your wallet
```

```
error: Invalid value for '<user-address>': bad hexadecimal sequence size
``` -->

## Mint token

You can issue amount of tokens determined by a `token-id` with the same token address as your user address.

```
intmax tx mint -i <token-id> --amount <amount>
```

### Example

Deposit your fungible token (the token address is the same with your user address and amount is an integer less than 2^56).

```
intmax tx mint --amount 1000000
```

### Example

You can issue NFT. The token id can be selected from 0x01 to 0xff.

```
intmax tx mint --nft -i 0x01
```

#### success response

```
deposit successfully
```

<!-- #### failure response

```
Error: you cannot omit --token-id attribute with --nft flag
```

```
Error: you cannot omit --amount attribute without --nft flag
```

```
Error: it is recommended that the NFT token ID be something other than 0x00
``` -->

## Send token

The token determined by `token-address` and `token-id` is transferred to the `receiver-address` in amount.
As in the case of issuing a token, if `token-address` is omitted, it is treated as the same address as the receiver's own address.

```
intmax tx send --receiver-address <receiver-address> [-a <token-address>] -i <token-id> --amount <amount>
```

### Example

Send your 1000000 fungible token to address `0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d`.

```
intmax tx send --receiver-address 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d --amount 1000000
```

### Example

Send your NFT (token ID is 0x01) to address `0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d`.

```
intmax tx send --receiver-address 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d --nft -i 0x01
```

#### success response

```
WARNING: DO NOT interrupt execution of this program while a transaction is being sent.
start proving: user_tx_proof
prove: 2.886 sec
transaction hash is 0x2d5b0466183df693bbad9660b687e8d51e163b19ebde11fc5ded24e5a33d5584
broadcast transaction successfully
start proving: received_signature
prove: 0.004 sec
send received signature successfully
```

<!--
#### failure response

```
Error: output asset amount is too much
```

```
Error: you cannot omit --token-id attribute with --nft flag
```

```
Error: you cannot omit --amount attribute without --nft flag
```

```
Error: it is recommended that the NFT token ID be something other than 0x00
``` -->

## Display assets

Displays all currently owned assets.

```sh
intmax account assets
```

### Example

#### success response

```
User: 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d
--------------------------------------------------------------------------------------
  No assets held
--------------------------------------------------------------------------------------
```

```
User: 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d
--------------------------------------------------------------------------------------
  Token Address | 0x3d4dcb332de1452f4f3de0612cb1c8a3ac892701f3e23627a634f2d962dc0712
  Token ID      | 0x01
  Amount        | 1
--------------------------------------------------------------------------------------
```

## Bulk-mint

You can issue new token according to the contents of the file. Up to 16 tokens can be sent together.
For more information, see [Bulk-transfer File Format](../tests/airdrop/README.md).

```
intmax tx bulk-mint -f <file-path>
```

### Example

```sh
intmax tx bulk-mint -f ./tests/airdrop/example2.csv
```

#### success response

```
deposit successfully
WARNING: DO NOT interrupt execution of this program while a transaction is being sent.
start proving: user_tx_proof
prove: 2.861 sec
transaction hash is 0xb71cf8dcd05da384dcba305ae5055a1ae0072572683be6dd11d7e5d74e2eb091
no purging transaction given
start proving: received_signature
prove: 0.003 sec
send received signature successfully
```

## Bulk-transfer

You can transfer owned tokens according to the contents of the file. Up to 8 tokens can be sent together.
For more information, see [Bulk-transfer File Format](../tests/airdrop/README.md).

```
intmax tx bulk-transfer -f <file-path>
```

### Example

```sh
intmax tx bulk-transfer -f ./tests/airdrop/example3.csv
```

#### success response

```
WARNING: DO NOT interrupt execution of this program while a transaction is being sent.
start proving: user_tx_proof
prove: 2.821 sec
transaction hash is 0x94039632d3af99ef06d750509d6153d1958720ead3f1dbd98f987059eea4eb5d
broadcast transaction successfully
start proving: received_signature
prove: 0.006 sec
send received signature successfully
```

<!-- #### failure response

```
Error: output asset amount is too much
``` -->
