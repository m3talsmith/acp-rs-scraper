Feature: Explore does not wait forever
  A LiveView join that never answers used to sit there while heartbeats kept
  the socket looking alive. Explore gives up, says so, and stores nothing.

  Background:
    Given a mock Red Star site

  Scenario: Explore stops when the site never answers the join
    Given the explore feed hangs
    When I run "--nick alice --no-mentions --no-attachments"
    Then it succeeds
    When I run "--explore"
    Then it fails
    And stderr contains "Already logged in."
    And stderr contains "Explore: requesting"
    And stderr contains "Explore: still waiting"
    And stderr contains "timed out"
    And stderr contains "Nothing was stored"
