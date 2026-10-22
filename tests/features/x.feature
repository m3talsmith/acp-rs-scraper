Feature: X is prepared and does not connect
  The read command stops before any request. Nothing here contacts x.com.

  Scenario: A handle read is refused
    When I run "--x-read alice"
    Then it fails
    And stderr contains "prepared, gated"
    And stderr contains "Nothing was fetched."
    And stdout does not contain "alice"

  Scenario: X is listed and starts off
    When I run "--services"
    Then it succeeds
    And stdout contains "redstar  Red Star  enabled"
    And stdout contains "x  X  disabled"

  Scenario: Enabling X writes the file and does not fetch
    When I run "--service-enable x"
    Then it succeeds
    And stdout contains "x enabled"
    And the file ".acp/services/x.toml" contains "enabled = true"
