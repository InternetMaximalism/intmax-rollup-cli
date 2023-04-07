import { ethers } from "hardhat";
import {
  loadAddressList,
  encodeIntmaxAddress,
  storeAddressList,
  networkName,
} from "./utils";

require("dotenv").config();

async function main() {
  let addressList;
  try {
    addressList = loadAddressList();
  } catch (err) {
    addressList = {};
  }

  console.log(addressList);

  const offerManagerAddress = addressList[networkName].offerManager;
  console.log(offerManagerAddress);

  // your intmax address
  const sellerIntmaxAddress = encodeIntmaxAddress(
    process.env.SELLER_INTMAX_ADDRESS!
  );
  console.log("sellerIntmaxAddress:", sellerIntmaxAddress);

  // asset ID you would like to sell (usually the same as `sellerIntmaxAddress`)
  const sellerAssetId = encodeIntmaxAddress(
    process.env.SELLER_ASSET_ID || sellerIntmaxAddress
  );
  const sellerAmount = 10;
  // This auction will close in 2 minutes.
  const auctionPeriodSec = 120;
  // The minimal bid amount is 0.0001 ETH.
  const minBidAmount = ethers.utils.parseEther("0.0001");

  const SimpleAuction = await ethers.getContractFactory("SimpleAuction");
  const simpleAuction = await SimpleAuction.deploy(
    offerManagerAddress,
    sellerIntmaxAddress,
    sellerAssetId,
    sellerAmount,
    auctionPeriodSec,
    minBidAmount
  );

  await simpleAuction.deployed();

  console.log(`Deploy a SimpleAuction contract: ${simpleAuction.address}`);

  addressList[networkName].simpleAuction = simpleAuction.address;

  storeAddressList(addressList);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
