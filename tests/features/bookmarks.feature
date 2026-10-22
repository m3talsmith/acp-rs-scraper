Feature: Bookmarks
  Bookmarks are kept locally. Syncing a site list is not in this binary.

  Background:
    Given a mock Red Star site

  Scenario: Bookmarking works offline
    When I run "--bookmark A1,A2"
    Then it succeeds
    And my bookmarks are "A1,A2"
    When I run "--unbookmark A2"
    Then my bookmarks are "A1"
    When I run "--list-bookmarks"
    Then stdout contains "A1"
    And stderr contains "1 bookmarks"

  Scenario: Invalid post ids are rejected
    When I run "--bookmark a/b"
    Then it fails

  Scenario: The TUI star runs a local bookmark job
    When I bookmark "A3" from the TUI
    Then it succeeds
    And my bookmarks are "A3"
