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
