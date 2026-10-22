Feature: An installed package is loaded and called

  Scenario: A verified package is loaded and a nick read calls it
    Given the fixture package is installed
    When I run "--plugins-reload"
    Then it succeeds
    And stdout contains "loaded fixture"
    When I run "--nick ada --no-mentions"
    Then it succeeds
    And post "fixture-1" has content "from the fixture"

  Scenario: A terminal UI scan calls the loaded package
    Given the fixture package is installed
    When I start scanning from the TUI with the nicks "ada" selected
    Then it succeeds
    And post "fixture-1" has content "from the fixture"

  Scenario: A plugin-crate mismatch is refused and the archive stays
    Given the mismatch package is installed
    When I run "--plugins-reload"
    Then it fails
    And stderr contains "core mismatch"
    And stdout does not contain "loaded"
    And the installed plugin archive is still in place

  Scenario: A bad sha256 is quarantined and never opened
    Given a fixture package with a bad sha256 is installed
    When I run "--plugins-reload"
    Then it fails
    And stderr contains "sha256 mismatch"
    And the quarantined plugin archive exists
    And the plugins directory has no archive
    When I run "--plugins-reload"
    Then it succeeds
    And stdout does not contain "loaded"

  Scenario: Reload replaces the loaded package
    Given the fixture package is installed
    When I run "--plugins-reload"
    Then it succeeds
    And stdout contains "loaded fixture"
    Given the reloaded fixture package replaces it
    When I run "--plugins-reload"
    Then it succeeds
    And stdout contains "loaded fixture"
    When I run "--nick ada --no-mentions"
    Then it succeeds
    And post "fixture-1" has content "from the reloaded fixture"
