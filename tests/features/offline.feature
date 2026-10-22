Feature: Offline search, export, import and maintenance
  Everything here works from the local database without logging in.

  Background:
    Given a mock Red Star site
    And I have imported the offline fixture

  Scenario: Search the local database
    When I run "--search strike"
    Then it succeeds
    And stdout contains "fifth general strike"

  Scenario: Export several formats at once
    When I run "--export $HOME/both.json --format json,html"
    Then it succeeds
    And the file "$HOME/both.json" exists
    And the file "$HOME/both.html" exists

  Scenario: Several formats cannot go to stdout
    When I run "--search strike --format json --format html"
    Then it fails
    And stderr contains "several formats need --output"

  Scenario: Export, clear everything, and import it back
    When I run "--export $HOME/all.json"
    Then it succeeds
    And the file "$HOME/all.json" exists
    When I run "--clear-all --yes"
    Then it succeeds
    And the database file is gone
    And the attachments directory is gone
    And my preferences are kept
    When I run "--import $HOME/all.json"
    Then it succeeds
    And the database has 7 posts

  Scenario: Remove a nick, their threads' replies and unreferenced files
    When I run "--remove-nick @alice --yes"
    Then it succeeds
    And the database has 0 posts by "alice"
    And post "R1" is not stored
    And the attachments table has 0 rows
    And 0 attachment files are on disk
    And stderr contains "removed:"

  Scenario: Maintenance refuses to run without a terminal or --yes
    When I run "--clear-all"
    Then it fails
    And stderr contains "pass --yes"

  Scenario: Export exactly the chosen posts (the TUI's export basket)
    When I run "--export $HOME/picked.json --post-ids A5,R1"
    Then it succeeds
    And the file "$HOME/picked.json" contains "fifth general strike"
    And the file "$HOME/picked.json" contains "bob replies to alice"
    And the file "$HOME/picked.json" does not contain "second"
    And stderr contains "2 chosen posts"

  Scenario: Unknown chosen posts are warned about
    When I run "--export $HOME/none.json --post-ids NOPE1234"
    Then it fails
    And stderr contains "post NOPE1234 is not in the database"

  Scenario: The TUI export basket runs a --post-ids export
    When I export the TUI basket "A2,A3" to "$HOME/basket.json"
    Then it succeeds
    And the file "$HOME/basket.json" contains "third"
