import { time, loadFixture } from "@nomicfoundation/hardhat-network-helpers";
import { expect } from "chai";
import { ethers } from "hardhat";

describe("SimpleAuction", function () {
  // We define a fixture to reuse the same setup in every test.
  // We use loadFixture to run this setup once, snapshot that state,
  // and reset Hardhat Network to that snapshot in every test.
  async function deployFixture() {
    // Contracts are deployed using the first signer/account by default
    const [owner, bidder1, bidder2] = await ethers.getSigners();

    const OfferManager = await ethers.getContractFactory("OfferManager");
    const offerManager = await OfferManager.deploy();

    await offerManager.deployed();

    const sellerIntmax =
      "0x0000000000000000000000000000000000000000000000000000000000000001";
    const sellerAssetId =
      "0x0000000000000000000000000000000000000000000000000000000000000001";
    const sellerAmount = 100;
    const auctionPeriodSec = 120;
    const minBidAmount = ethers.utils.parseEther("0.0001");

    const SimpleAuction = await ethers.getContractFactory("SimpleAuction");
    const simpleAuction = await SimpleAuction.deploy(
      offerManager.address,
      sellerIntmax,
      sellerAssetId,
      sellerAmount,
      auctionPeriodSec,
      minBidAmount
    );

    return {
      offerManager,
      simpleAuction,
      owner,
      bidder1,
      bidder2,
    };
  }

  describe("Deployment", function () {
    it("Should set the right maker and taker", async function () {
      const { offerManager, simpleAuction } = await loadFixture(deployFixture);

      const offerId = await simpleAuction.offerId();
      const [maker, , , , taker] = await offerManager.getOffer(offerId);

      expect(maker).to.equal(simpleAuction.address);
      expect(taker).to.equal(simpleAuction.address);
    });
  });

  describe("Bid", function () {
    it("Should not bid lower amount than `largestBidAmount`", async function () {
      const { simpleAuction, bidder1 } = await loadFixture(deployFixture);

      const bidderIntmax =
        "0x0000000000000000000000000000000000000000000000000000000000000002";
      const bidAmount = ethers.utils.parseEther("0.0001");
      await expect(
        simpleAuction.connect(bidder1).bid(bidderIntmax, {
          value: bidAmount,
        })
      ).to.be.revertedWith("should be larger than the previous bid");
    });

    it("Should set the right bidder and bid amount", async function () {
      const { simpleAuction, bidder1 } = await loadFixture(deployFixture);

      const bidderIntmax =
        "0x0000000000000000000000000000000000000000000000000000000000000002";
      const bidAmount = ethers.utils.parseEther("0.0002");
      await expect(
        simpleAuction.connect(bidder1).bid(bidderIntmax, {
          value: bidAmount,
        })
      ).not.to.be.reverted;

      expect(await simpleAuction.largestBidder()).to.equal(bidder1.address);
      expect(await simpleAuction.largestBidAmount()).to.equal(bidAmount);

      expect(await ethers.provider.getBalance(simpleAuction.address)).to.equal(
        bidAmount
      );
    });
  });

  describe("Claim", function () {
    it("Should not invoke `claim` function before closing auction", async function () {
      const { simpleAuction, bidder1 } = await loadFixture(deployFixture);

      const bidderIntmax =
        "0x0000000000000000000000000000000000000000000000000000000000000002";
      const bidAmount = ethers.utils.parseEther("0.0002");

      await simpleAuction.connect(bidder1).bid(bidderIntmax, {
        value: bidAmount,
      });

      await expect(simpleAuction.claim()).to.be.revertedWith(
        "this auction is not closed"
      );
    });

    it("Should emit `Activate` events when someone invoke `claim` function", async function () {
      const { offerManager, simpleAuction, bidder1 } = await loadFixture(
        deployFixture
      );

      const bidderIntmax =
        "0x0000000000000000000000000000000000000000000000000000000000000002";
      const bidAmount = ethers.utils.parseEther("0.0002");

      await simpleAuction.connect(bidder1).bid(bidderIntmax, {
        value: bidAmount,
      });

      const closingTime = await simpleAuction.closingTime();
      await time.increaseTo(closingTime);

      const offerId = await simpleAuction.offerId();
      await expect(simpleAuction.claim())
        .to.emit(offerManager, "OfferActivated")
        .withArgs(offerId, bidderIntmax);
    });

    it("Should fail to invoke `activate` function directly", async function () {
      const { offerManager, simpleAuction, bidder1 } = await loadFixture(
        deployFixture
      );

      const takerAmount = ethers.utils.parseEther("0.0001");
      const bidderIntmax =
        "0x0000000000000000000000000000000000000000000000000000000000000002";
      const bidAmount = ethers.utils.parseEther("0.0002");

      await simpleAuction.connect(bidder1).bid(bidderIntmax, {
        value: bidAmount,
      });

      const offerId = await simpleAuction.offerId();
      await expect(
        offerManager.connect(bidder1).activate(offerId, {
          value: takerAmount,
        })
      ).to.be.revertedWith("offers can be activated by its taker");
    });

    it("Can invoke `claim` function even if no one is participating in this auction", async function () {
      const { offerManager, simpleAuction } = await loadFixture(deployFixture);

      const closingTime = await simpleAuction.closingTime();
      await time.increaseTo(closingTime);

      const offerId = await simpleAuction.offerId();
      await expect(simpleAuction.claim())
        .to.emit(offerManager, "OfferDeactivated")
        .withArgs(offerId);
    });
  });

  describe("Withdraw", function () {
    it("in the case of no bidders", async function () {
      const { simpleAuction, owner } = await loadFixture(deployFixture);

      const closingTime = await simpleAuction.closingTime();
      await time.increaseTo(closingTime);

      await simpleAuction.claim();

      expect(await simpleAuction.withdrawableAmount(owner.address)).to.equal(
        "0"
      );

      await expect(simpleAuction.connect(owner).withdraw()).not.to.be.reverted;
    });

    it("in the case of 1 bidder", async function () {
      const { simpleAuction, owner, bidder1 } = await loadFixture(
        deployFixture
      );

      const bidderIntmax =
        "0x0000000000000000000000000000000000000000000000000000000000000002";
      const bidAmount = ethers.utils.parseEther("0.0002");

      await expect(
        simpleAuction.connect(bidder1).bid(bidderIntmax, {
          value: bidAmount,
        })
      ).not.to.be.reverted;

      const closingTime = await simpleAuction.closingTime();
      await time.increaseTo(closingTime);

      await expect(simpleAuction.claim()).not.to.be.reverted;

      expect(await simpleAuction.withdrawableAmount(owner.address)).to.equal(
        bidAmount
      );

      await expect(simpleAuction.connect(owner).withdraw()).not.to.be.reverted;

      expect(await simpleAuction.withdrawableAmount(bidder1.address)).to.equal(
        "0"
      );
    });

    it("in the case of 2 bidders", async function () {
      const { simpleAuction, owner, bidder1, bidder2 } = await loadFixture(
        deployFixture
      );

      const bidderIntmax =
        "0x0000000000000000000000000000000000000000000000000000000000000002";
      const bidAmount = ethers.utils.parseEther("0.0002");

      await expect(
        simpleAuction.connect(bidder1).bid(bidderIntmax, {
          value: bidAmount,
        })
      ).not.to.be.reverted;

      const bidder2Intmax =
        "0x0000000000000000000000000000000000000000000000000000000000000003";
      const bid2Amount = ethers.utils.parseEther("0.0004");

      await expect(
        simpleAuction.connect(bidder2).bid(bidder2Intmax, {
          value: bid2Amount,
        })
      ).not.to.be.reverted;

      const closingTime = await simpleAuction.closingTime();
      await time.increaseTo(closingTime);

      await expect(simpleAuction.claim()).not.to.be.reverted;

      expect(await simpleAuction.withdrawableAmount(owner.address)).to.equal(
        bid2Amount
      );

      await expect(simpleAuction.connect(owner).withdraw()).not.to.be.reverted;

      expect(await simpleAuction.withdrawableAmount(bidder1.address)).to.equal(
        bidAmount
      );

      await expect(simpleAuction.connect(bidder1).withdraw()).not.to.be
        .reverted;

      expect(await simpleAuction.withdrawableAmount(bidder2.address)).to.equal(
        "0"
      );
    });
  });
});
