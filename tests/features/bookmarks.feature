Feature: Bookmarks
  Bookmarks are kept locally (they work offline) and --sync-bookmarks
  merges them with the site's own bookmarks: it reads the site's list,
  stores those posts, and pushes local additions and removals.

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

  Scenario: Syncing pulls the site's bookmarks and their posts
    When I run "--sync-bookmarks"
    Then it succeeds
    And my bookmarks are "C1"
    And post "C1" is stored once

  Scenario: Syncing pushes local changes to the site
    Given I have run "--sync-bookmarks"
    And I have run "--bookmark A1"
    And I have run "--unbookmark C1"
    When I run "--sync-bookmarks"
    Then it succeeds
    And the site has bookmarks "A1"
    And my bookmarks are "A1"
    And stderr contains "1 added and 1 removed on the site"

  Scenario: Invalid post ids are rejected
    When I run "--bookmark a/b"
    Then it fails

  Scenario: The TUI star runs a local bookmark job
    When I bookmark "A3" from the TUI
    Then it succeeds
    And my bookmarks are "A3"
