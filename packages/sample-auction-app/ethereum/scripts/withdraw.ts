import { ethers } from "hardhat";
import { loadAddressList, networkName } from "./utils";

require("dotenv").config();

async function main() {
  const addressList = loadAddressList();
  const simpleAuctionAddress = addressList[networkName].simpleAuction;

  const SimpleAuction = await ethers.getContractFactory("SimpleAuction");
  const simpleAuction = SimpleAuction.attach(simpleAuctionAddress);
  console.log(`Load a SimpleAuction contract: ${simpleAuction.address}`);

  await simpleAuction.withdraw();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
