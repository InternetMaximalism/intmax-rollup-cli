import { loadAddressList, networkName, storeAddressList } from "./utils";

require("dotenv").config();

async function main() {
  let addressList;
  try {
    addressList = loadAddressList();
  } catch (err) {
    addressList = {};
  }
  console.log(addressList);

  let offerManagerAddress;
  let reverseOfferManagerAddress;
  if (networkName === "scrollalpha") {
    // Scroll alpha
    offerManagerAddress = "0x007c969728eE4f068ceCF3405D65a037dB5BeEa1";
    reverseOfferManagerAddress = "0x4ee8cB7864df06A8c7703988C15bAaAB9ac47CAe";
  } else if (networkName === "polygonzkevm") {
    // Polygon zkEVM
    offerManagerAddress = "0x161a72Bc1b76586a36A9014Dd58d401eE2B24094";
    reverseOfferManagerAddress = "0x1E316b313de98C7eCb2393995ef27715E3E1c7a7";
  } else {
    // TODO: Deploy OfferManager contract.
    throw new Error("Please use deploy.ts");
  }

  addressList[networkName] = {
    offerManager: offerManagerAddress,
    reverseOfferManager: reverseOfferManagerAddress,
  };

  storeAddressList(addressList);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
