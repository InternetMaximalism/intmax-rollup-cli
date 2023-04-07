import { ethers } from "hardhat";

async function main() {
  const OfferManager = await ethers.getContractFactory("OfferManager");
  const offerManager = await OfferManager.deploy();

  await offerManager.deployed();

  console.log(`Deploy a OfferManager contract: ${offerManager.address}`);

  const sellerL2Address =
    "0x0000000000000000000000000000000000000000000000000000000000000001";
  const sellerAssetId =
    "0x0000000000000000000000000000000000000000000000000000000000000001";
  const sellerAmount = 100;
  const auctionPeriodSec = 120;
  const minBidAmount = ethers.utils.parseEther("0.0001");

  const SimpleAuction = await ethers.getContractFactory("SimpleAuction");
  const simpleAuction = await SimpleAuction.deploy(
    offerManager.address,
    sellerL2Address,
    sellerAssetId,
    sellerAmount,
    auctionPeriodSec,
    minBidAmount
  );

  await simpleAuction.deployed();

  console.log(`Deploy a SimpleAuction contract: ${simpleAuction.address}`);
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
