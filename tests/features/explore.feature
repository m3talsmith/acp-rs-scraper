Feature: Explore is not in this binary
  Asking for a site feed fails before any connection.

  Background:
    Given a mock Red Star site

  Scenario: Explore refuses
    When I run "--explore"
    Then it fails
    And stderr contains "not in this binary"
