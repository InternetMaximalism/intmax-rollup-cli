# Bulk-transfer File Format

Writing CSV allows you to transfer tokens together.

NOTICE: Account nicknames cannot be used in the CSV file.

## Sending (fungible) token

```csv
Token Address, Recipient, Fungibility, Token ID (NFT), Amount (FT)
0xc71603f33a1144ca7953db0ab48808f4c4055e3364a246c33c18a9786cb0b359, 0xcf7d8efa32b26631428a3cc177653a1256fd49ffd49c1ae8cc62802450065b0a, FT, , 9000000
```

- `Token Address`: token address (Default: my user address)
- `Recipient`: recipient address
- `Fungibility`: `FT` or `NFT`
- `Token ID`: you can omit this field if this token is fungible (default: `0x00`)
- `Amount`: sending amount less than 2^56

## Sending NFT

```csv
Token Address, Recipient, Fungibility, Token ID (NFT), Amount (FT)
0xc71603f33a1144ca7953db0ab48808f4c4055e3364a246c33c18a9786cb0b359, 0x714bdc6f38947e6da5ee9596c50b2e06e4e01c8885f98cf29d9c2f656eb3b45d, NFT, 0x01,
```

- `Token Address`: token address (Default: my user address)
- `Recipient`: recipient address
- `Fungibility`: `FT` or `NFT`
- `Token ID`: token ID selected from `0x01` to `0xff`
- `Amount`: you can omit this field if this token is non-fungible (default: 1)
