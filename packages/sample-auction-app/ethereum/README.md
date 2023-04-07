# Simple Auction with intmax



## How to test

See [intmax-rollup-cli](https://github.com/InternetMaximalism/intmax-rollup-cli-flag/blob/main/README.md) and build CLI.
First, create the accounts required for the test.

```sh
intmax account add --default --nickname seller
intmax account add --nickname buyer
```

If you execute the above command, you will get the following response:

```txt
new account added: <seller-intmax-address>
the above account appears replaced by seller
new account added: <buyer-intmax-address>
the above account appears replaced by buyer
```

Please note the address written in `<seller-intmax-address>` and `<buyer-intmax-address>` section.
The seller issues a "seller" token and transfers it to the zero address.

```sh
intmax tx mint --amount 1 -i 0x00
intmax tx send --amount 1 -i 0x00 --receiver-address 0x0000000000000000
```

Go to the directory where the sample auction code is located.

```sh
# Change the path according to your environment.
cd /path/to/sample-auction-app/ethereum
```

Install modules and compile contracts.

```
npm i
npx hardhat compile
```

Replace `<seller-intmax-address>` and `<buyer-intmax-address>` section with the address you have created before running the following commands.

```sh
# The seller puts a "seller" token up for auction.
SELLER_INTMAX_ADDRESS=<seller-intmax-address> npx hardhat --network scrollalpha run ./scripts/start_auction.ts
# The buyer bids 0.0002 ETH for seller's prize.
# The bid must be higher than either the minimum bid set by the seller or the maximum bid so far.
BUYER_INTMAX_ADDRESS=<buyer-intmax-address> npx hardhat --network scrollalpha run ./scripts/bid.ts
# See auction information.
npx hardhat --network scrollalpha run ./scripts/get_auction_info.ts
# The buyer claim auction prizes.
npx hardhat --network scrollalpha run ./scripts/claim.ts
# The seller withdraw the amount bid from the buyer.
npx hardhat --network scrollalpha run ./scripts/withdraw.ts
```

You can see that the buyer balance changes before and after the offer is activated.

```sh
intmax account assets -u buyer
intmax io activate <offer-id> --network scroll
intmax account assets -u buyer
```

Success if the following is displayed.

```txt
User: buyer
--------------------------------------------------------------------------------------
  No assets held
--------------------------------------------------------------------------------------

given offer ID is already activated

User: buyer
--------------------------------------------------------------------------------------
  Token Address | [seller]
  Token ID      | 0x00
  Amount        | 10
--------------------------------------------------------------------------------------
```
