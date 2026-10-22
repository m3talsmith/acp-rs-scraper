Feature: Feeds
  A feed is rules over the local database. Sharing writes the rules only.
  Refresh asks first and does not connect on a dry run.

  Background:
    Given a mock Red Star site
    And I have run "--nick alice --no-mentions"

  Scenario: Create a feed, view it, and export the posts
    When I run "--feed-create Theory --feed-description Books"
    Then it succeeds
    And stdout contains "feed Theory created"
    When I run "--feed-add Theory --feed-nick alice"
    Then it succeeds
    And stdout contains "rules=1"
    When I run "--feed-show Theory"
    Then it succeeds
    And stdout contains "feed Theory matches="
    When I run "--feed-export Theory --format json --output $HOME/theory.json"
    Then it succeeds
    And the file "$HOME/theory.json" exists

  Scenario: Share the rules and import them under a new name
    When I run "--feed-create Shop"
    Then it succeeds
    When I run "--feed-add Shop --feed-tag strike"
    Then it succeeds
    When I run "--feed-share Shop --share-string --output $HOME/shop.acpfeed"
    Then it succeeds
    And stdout contains "acpfeed1:"
    And the file "$HOME/shop.acpfeed" exists
    When I run "--feed-import $HOME/shop.acpfeed --feed-name Shop2"
    Then it succeeds
    And stdout contains "feed Shop2 imported"
    When I run "--feed-show Shop2"
    Then it succeeds

  Scenario: Refresh prints the plan and does not connect
    When I run "--feed-create Theory"
    Then it succeeds
    When I run "--feed-add Theory --feed-nick alice --feed-tag strike"
    Then it succeeds
    When I run "--feed-refresh Theory --dry-run"
    Then it succeeds
    And stdout contains "dry-run: no connection opened"
    And stdout contains "refresh nick alice"
    And stdout contains "refresh tag strike"
    And stdout does not contain "https://acp.us"

  Scenario: Delete asks first
    When I run "--feed-create Theory"
    Then it succeeds
    When I run "--feed-delete Theory"
    Then it succeeds
    And stdout contains "nothing deleted"
    When I run "--feed-list"
    Then it succeeds
    And stdout contains "feed Theory"
    When I run "--feed-delete Theory --yes"
    Then it succeeds
    And stdout contains "feed Theory deleted"
