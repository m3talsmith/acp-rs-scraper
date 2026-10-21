Feature: Spaces
  The Spaces list is read-only. A live room shows a live badge. A LiveKit
  token is not stored, logged, or printed. Joining a room is not built.

  Background:
    Given a mock Red Star site

  Scenario: --spaces lists live, scheduled, and ended rooms
    When I run "--spaces"
    Then it succeeds
    And stdout contains "LIVE"
    And stdout contains "strike-meeting"
    And stdout contains "scheduled"
    And stdout contains "ended"
    And stdout contains "https://example.test/recordings/strike"
    And stdout does not contain "lk-secret-token-do-not-keep"
    And stdout does not contain "wss://"
    And the home has no livekit token

  Scenario: a second --spaces stops when the page is already known
    Given I have run "--spaces"
    When I run "--spaces"
    Then it succeeds
    And stderr contains "all known"
    And stdout does not contain "lk-secret-token-do-not-keep"
