import { ethers } from "hardhat";
import { decodeIntmaxAddress, loadAddressList, networkName } from "./utils";

require("dotenv").config();

async function main() {
  const addressList = loadAddressList();
  const offerManagerAddress = addressList[networkName].offerManager;
  const simpleAuctionAddress = addressList[networkName].simpleAuction;

  const OfferManager = await ethers.getContractFactory("OfferManager");
  const offerManager = OfferManager.attach(offerManagerAddress);
  console.log(`Load a OfferManager contract: ${offerManager.address}`);

  const SimpleAuction = await ethers.getContractFactory("SimpleAuction");
  const simpleAuction = SimpleAuction.attach(simpleAuctionAddress);
  console.log(`Load a SimpleAuction contract: ${simpleAuction.address}`);

  const isClaimed = await simpleAuction.done();
  if (!isClaimed) {
    await simpleAuction.claim();
  }

  const offerId = await simpleAuction.offerId();

  const offer = await offerManager.getOffer(offerId);
  const [
    maker,
    makerIntmaxAddress,
    makerAssetId,
    makerAmount,
    taker,
    takerIntmaxAddress,
    takerTokenAddress,
    takerAmount,
    activated,
  ] = offer;

  console.log("".padEnd(64, "="));
  console.log(
    `If you are the owner of address ${decodeIntmaxAddress(
      takerIntmaxAddress
    )} on intmax, `
  );
  console.log("you can receive auction prizes by the following command:\n");
  console.log(
    `$ intmax io activate ${offerId.toString()} --network ${networkName}`
  );
  console.log(
    `$ intmax account assets -u ${decodeIntmaxAddress(takerIntmaxAddress)}`
  );
  console.log("".padEnd(64, "="));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
