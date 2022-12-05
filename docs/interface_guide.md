# Interface Guide

This page describes how to use it.

## Create account

Creates a new account. If the default flag is given, it is used if user-address is omitted in commands that require it. A hex string with 0x-prefix of less than or equal to 32 bytes can be given as private key.

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

## Mint token

You can issue amount of tokens determined by a `token-id` with the same `contract-address` as your address.

```
intmax deposit -i <token-id> --amount <amount>
```

### Example

Deposit your assets (the token contract address is the same with your address, the token id can be selected 0x00 - 0xff and amount is an integer less than 2^56)

```
intmax deposit -i 0x00 --amount 10
```

### Merge token

Receive issued or transferred tokens and reflect them as their own assets.

```sh
intmax tx merge
```

## Send token

The token determined by `contract-address` and `token-id` is transferred to the `receiver-address` in amount. As in the case of issuing a token, if `contract-address` is omitted, it is treated as the same address as the receiver's own address.

```
intmax tx send --receiver-address <receiver-address> [--contract-address <contract-address>] -i <token-id> --amount <amount>
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
