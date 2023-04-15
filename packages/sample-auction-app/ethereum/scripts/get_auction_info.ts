import { ethers } from "hardhat";
import { decodeIntmaxAddress, loadAddressList, networkName } from "./utils";

require("dotenv").config();

async function main() {
  const addressList = loadAddressList();

  const offerManagerAddress = addressList[networkName].offerManager;
  const OfferManager = await ethers.getContractFactory("OfferManager");
  const offerManager = OfferManager.attach(offerManagerAddress);
  console.log(`Load a OfferManager contract: ${offerManager.address}`);

  const simpleAuctionAddress = addressList[networkName].simpleAuction;
  const SimpleAuction = await ethers.getContractFactory("SimpleAuction");
  const simpleAuction = SimpleAuction.attach(simpleAuctionAddress);
  console.log(`Load a SimpleAuction contract: ${simpleAuction.address}`);

  const offerId = await simpleAuction.offerId();
  console.log("offerId:", offerId.toString());

  const offer = await offerManager.getOffer(offerId);
  console.log(
    "sellerIntmaxAddress:",
    decodeIntmaxAddress(offer.makerIntmaxAddress)
  );

  const largestBidder = await simpleAuction.largestBidder();
  console.log("largestBidder:", largestBidder);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
