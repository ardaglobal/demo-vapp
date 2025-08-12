// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title EventHelpers
/// @notice Helper functions for the comprehensive event system
abstract contract EventHelpers {
    
    /*//////////////////////////////////////////////////////////////
                        EVENT FILTERING HELPERS
    //////////////////////////////////////////////////////////////*/

    /// @notice Get event statistics for monitoring.
    function getEventStats() external view virtual returns (EventStats memory);

    /// @notice Get event count by submitter address.
    function getEventCountBySubmitter(address submitter) external view virtual returns (uint256);

    /// @notice Get event count by state ID.
    function getEventCountByStateId(bytes32 stateId) external view virtual returns (uint256);

    /// @notice Get daily event count for a specific day.
    function getDailyEventCount(uint256 day) external view virtual returns (uint256);

    /// @notice Toggle read event tracking (gas optimization).
    function setReadEventTracking(bool enabled) external virtual;

    /// @notice Get events in a time range (helper for filtering).
    function getTimeRangeInfo(uint256 startTime, uint256 endTime) external pure returns (
        bool isValid,
        uint256 dayCount
    ) {
        if (startTime >= endTime) {
            return (false, 0);
        }
        
        uint256 startDay = startTime / 86400;
        uint256 endDay = endTime / 86400;
        dayCount = endDay - startDay + 1;
        isValid = true;
    }

    /// @notice Get aggregated event counts for a time range.
    function getEventCountInRange(uint256 startTime, uint256 endTime) external view virtual returns (
        uint256 totalEvents
    );

    /*//////////////////////////////////////////////////////////////
                        INTERNAL EVENT HELPERS
    //////////////////////////////////////////////////////////////*/

    /// @notice Internal function to update event statistics.
    function _updateEventStats(string memory eventType) internal virtual;

    /// @notice Internal function to track read events.
    function _trackReadEvent(bytes32 stateId, bytes32 proofId) internal virtual;
    
    /// @notice Event statistics structure
    struct EventStats {
        uint256 totalStateUpdates;
        uint256 totalBatchUpdates;
        uint256 totalProofStored;
        uint256 totalProofVerified;
        uint256 totalStateReads;
        uint256 totalProofReads;
        uint256 lastEventTimestamp;
    }
}