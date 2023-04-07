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
    offerManagerAddress = "0xD8f7FbABEacD6f103FB7278b3a7106e2fFBF0763";
    reverseOfferManagerAddress = "0x2D372972f8c325dEFD7252c0e7d8cBd09a8A4c67";
  } else if (networkName === "polygonzkevm") {
    // Polygon zkEVM
    offerManagerAddress = "0x5602c213E1aEe9159E2A4d11fbFe19C56E7B3bE1";
    reverseOfferManagerAddress = "0xD9626E93c03E83647b1202a4a1CA96Bcc399F9E7";
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
