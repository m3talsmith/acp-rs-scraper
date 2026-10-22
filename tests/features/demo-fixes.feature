Feature: Demo fixes
  Bugs found while recording the demo. These scenarios stay on the mock site
  or on the in-process terminal UI. They do not open acp.us.

  Background:
    Given a mock Red Star site

  Scenario: Help search keeps j and k
    When I search Help for "basket"
    Then the Help text contains "Search: basket"
    And the Help text contains "Basket"

  Scenario: Sorting a thread while logged out stays on this machine
    When I sort an open thread while logged out
    Then sorting stays on this machine

  Scenario: A neutral brand does not show the ACP wording
    When I open Help on the brand "plain"
    Then the screen contains "Red Star is a local client"
    And the screen does not contain "ACP Red Star"
    And the screen does not contain "ACP platform"
    And the screen does not contain "Sync with acp.us"
    And the screen does not contain "The future belongs to the working class"
    When I open Help on the brand "default"
    Then the screen contains "ACP Red Star is a local client"
    And the screen contains "ACP platform"
    And the screen contains "Sync with acp.us"
    When I open About on the brand "plain"
    Then the screen contains "Red Star"
    And the screen does not contain "ACP platform"
    When I open Bookmarks on the brand "plain"
    Then the screen does not contain "Sync with acp.us"
    When I open the theme editor preview on the brand "plain"
    Then the theme preview does not contain "May Day"
    And the theme preview does not contain "Workers of the world"
    And the theme preview does not contain "Strike fund"
    And the theme preview does not contain "Solidarity"
    When I open the theme editor preview on the brand "default"
    Then the theme preview contains "May Day"
    And the theme preview contains "Strike fund"
    When I run "--brand plain --list-brands"
    Then it succeeds
    And stdout contains "plain"
    And stdout contains "(no motto)"
    And stdout contains "default"
    And stdout contains "The future belongs to the working class"
