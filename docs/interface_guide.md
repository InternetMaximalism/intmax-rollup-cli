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

## Display accounts

You can check the account list.

```sh
intmax account list
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

## Export private key

You can export your private key to a specified file.

```
intmax account export -f <file-path>
```

### Example

```sh
intmax account export -f ~/Documents/my-account
```

## Mint token

You can issue amount of tokens determined by a `token-id` with the same token address as your user address.

```
intmax deposit -i <token-id> --amount <amount>
```

### Example

Deposit your token (the token address is the same with your user address, the token id can be selected from 0x00 to 0xff and amount is an integer less than 2^56)

```
intmax deposit -i 0x00 --amount 10
```

### Merge token

Receive issued or transferred tokens and reflect them as their own assets.

```sh
intmax tx merge
```

## Send token

The token determined by `token-address` and `token-id` is transferred to the `receiver-address` in amount.
As in the case of issuing a token, if `token-address` is omitted, it is treated as the same address as the receiver's own address.

```
intmax tx send --receiver-address <receiver-address> [-a <token-address>] -i <token-id> --amount <amount>
```

### Example

```
intmax tx send --receiver-address 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d -i 0x00 --amount 10
```

## Display assets

Displays all currently owned assets. However, assets that have not yet been merged are not counted.

```sh
intmax assets
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
