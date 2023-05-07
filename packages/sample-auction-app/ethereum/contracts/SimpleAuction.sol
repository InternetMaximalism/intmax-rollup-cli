// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.17;

import "@intmax/interoperability-contracts/contracts/OfferManagerV2.sol";
import "@openzeppelin/contracts/utils/Context.sol";

contract SimpleAuction is Context {
    OfferManagerV2Interface _offerManagerInterface;
    uint256 public offerId;
    uint256 public closingTime;
    bool public done;
    address public beneficiary;
    address public largestBidder;
    uint256 public largestBidAmount;
    mapping(address => uint256) public withdrawableAmount;

    event TokenWithdrawn(address indexed claimer, uint256 amount);

    constructor(
        address offerManagerInterface,
        bytes32 sellerIntmax,
        uint256 sellerAssetId,
        uint256 sellerAmount,
        uint256 auctionPeriodSec,
        uint256 minBidAmount,
        bytes memory witness
    ) {
        _offerManagerInterface = OfferManagerV2Interface(offerManagerInterface);

        offerId = _offerManagerInterface.register(
            sellerIntmax,
            sellerAssetId,
            sellerAmount,
            address(this),
            sellerIntmax, // non-zero
            address(0), // ETH
            minBidAmount,
            witness
        );

        closingTime = block.timestamp + auctionPeriodSec;
        beneficiary = _msgSender();
        largestBidAmount = minBidAmount;
    }

    receive() external payable {}

    /**
     * You can bid in the auction with the amount of ETH sent with this transaction.
     * The amount may only be refunded if the bid is not successful.
     * @param bidderIntmax is a bidder L2 address.
     */
    function bid(bytes32 bidderIntmax) external payable {
        require(block.timestamp < closingTime, "this auction has already done");

        // This function can only be performed if the bid is larger than the previous bid.
        require(
            msg.value > largestBidAmount,
            "should be larger than the previous bid"
        );

        require(_msgSender() != address(0));

        // Allow refunds to previous bidders.
        if (largestBidder != address(0)) {
            withdrawableAmount[largestBidder] += largestBidAmount;
        }

        // Update the bid.
        largestBidder = _msgSender();
        largestBidAmount = msg.value;
        _offerManagerInterface.updateTaker(offerId, bidderIntmax);
    }

    /**
     * The winner can claim the tokens in the auction.
     */
    function claim() external {
        require(block.timestamp >= closingTime, "this auction is not closed");
        require(!done, "already claimed");
        done = true;

        if (largestBidder == address(0)) {
            bool success = _offerManagerInterface.deactivate(offerId);
            require(success, "fail to deactivate offer");
        } else {
            withdrawableAmount[beneficiary] += largestBidAmount;

            // NOTE: This contract sends ETH to OfferManager when activated, but it is refunded to this contract.
            // So this contract doesn't have to send largestBidAmount to OfferManager.
            uint256 minBidAmount = _offerManagerInterface
                .offers(offerId)
                .takerAmount;
            bool success = _offerManagerInterface.activate{value: minBidAmount}(
                offerId
            );
            require(success, "fail to activate offer");
        }
    }

    /**
     * The loser can be refunded his bid amount and the beneficiary can receive the largest bid amount.
     */
    function withdraw() external {
        uint256 withdrawnAmount = withdrawableAmount[_msgSender()];

        if (withdrawnAmount != 0) {
            withdrawableAmount[_msgSender()] = 0;
            (bool success, ) = payable(_msgSender()).call{
                value: withdrawnAmount
            }("");
            require(success, "fail to withdraw");
            emit TokenWithdrawn(_msgSender(), withdrawnAmount);
        }
    }
}
