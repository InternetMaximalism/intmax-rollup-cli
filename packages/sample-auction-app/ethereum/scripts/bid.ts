import { ethers } from "hardhat";
import { loadAddressList, encodeIntmaxAddress, networkName } from "./utils";

require("dotenv").config();

async function main() {
  const addressList = loadAddressList();
  const simpleAuctionAddress = addressList[networkName].simpleAuction;

  const SimpleAuction = await ethers.getContractFactory("SimpleAuction");
  const simpleAuction = SimpleAuction.attach(simpleAuctionAddress);
  console.log(`Load a SimpleAuction contract: ${simpleAuction.address}`);

  const bidderIntmaxAddress = encodeIntmaxAddress(
    process.env.BUYER_INTMAX_ADDRESS!
  );
  console.log("bidderIntmaxAddress:", bidderIntmaxAddress);
  const bidAmount = ethers.utils.parseEther("0.0002"); // >= minBidAmount

  await simpleAuction.bid(bidderIntmaxAddress, {
    value: bidAmount,
  });
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
