Feature: Terminal UI entry and the menu → command-line bridge
  A bare invocation opens the TUI; flags keep the headless CLI. Everything the
  TUI changes runs the headless CLI with the argv its menu builds, so these
  scenarios run exactly those argvs against the mock site.

  Background:
    Given a mock Red Star site

  Scenario: A bare run needs a terminal and points at the headless flags
    When I run ""
    Then it fails
    And stderr contains "the terminal UI needs a terminal"
    And stderr contains "--scrape"

  Scenario: --tui and logging-only flags also open the TUI
    When I run "--tui"
    Then it fails
    And stderr contains "the terminal UI needs a terminal"
    When I run "-v"
    Then it fails
    And stderr contains "the terminal UI needs a terminal"

  Scenario: --scrape does not fetch
    When I run "--scrape --no-mentions"
    Then it fails
    And stderr contains "not in this binary"

  Scenario: --scrape with nicks does not fetch
    When I run "--scrape --nick alice --no-mentions"
    Then it fails
    And stderr contains "not in this binary"

  Scenario: Start Scanning does not fetch
    Given the TUI Scrape menu has "Mentions" off
    When I start scanning from the TUI with the nicks "alice" selected
    Then it fails
    And stderr contains "not in this binary"

  Scenario: Bulk export from the TUI selection
    Given I have imported the offline fixture
    When I export the TUI selection "alice" to "$HOME/sel.json" as "json,html"
    Then it succeeds
    And the file "$HOME/sel.json" exists
    And the file "$HOME/sel.html" exists
    And the file "$HOME/sel.json" contains "alice"

  Scenario: Remove from the Maintain menu (after the TUI's confirmation)
    Given I have imported the offline fixture
    When I remove the TUI selection "alice" from the Maintain menu
    Then it succeeds
    And the database has 0 posts by "alice"
