import { HardhatUserConfig } from "hardhat/config";
import "@nomicfoundation/hardhat-toolbox";

require("dotenv").config();

const PRIVATE_KEY = process.env.PRIVATE_KEY!;

const config: HardhatUserConfig = {
  solidity: "0.8.17",
  defaultNetwork: "hardhat",
  networks: {
    hardhat: {},
    scrollalpha: {
      url: `https://alpha-rpc.scroll.io/l2`,
      chainId: 534353,
      accounts: PRIVATE_KEY ? [PRIVATE_KEY] : [],
    },
    polygonzkevmtest: {
      url: `https://rpc.public.zkevm-test.net`,
      chainId: 1442,
      accounts: PRIVATE_KEY ? [PRIVATE_KEY] : [],
    },
  },
};

export default config;
