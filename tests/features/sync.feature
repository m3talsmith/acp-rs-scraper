Feature: Syncing with the site
  --sync brings the database up to date: bookmarks both ways, then an
  incremental scrape of the given nicks. It writes no export unless asked.

  Background:
    Given a mock Red Star site

  Scenario: Sync bookmarks and nicks without printing an export
    Given I have run "--bookmark A2"
    When I run "--sync --nick alice --no-mentions"
    Then it succeeds
    And the database has 6 posts by "alice"
    And post "C1" is stored once
    And the site has bookmarks "C1,A2"
    And stdout does not contain "alice"

  Scenario: Sync with nothing but bookmarks
    When I run "--sync"
    Then it succeeds
    And my bookmarks are "C1"
    And the database has 0 posts by "alice"

  Scenario: Skip the bookmarks
    When I run "--sync --no-bookmarks --nick alice --no-mentions"
    Then it succeeds
    And post "C1" is not stored

  Scenario: --no-bookmarks needs --sync
    When I run "--no-bookmarks"
    Then it fails

  Scenario: Sync now from the TUI runs the sync the Sync menu describes
    When I sync from the TUI with the nicks "alice" selected
    Then it succeeds
    And the database has 6 posts by "alice"
    And my bookmarks are "C1"
