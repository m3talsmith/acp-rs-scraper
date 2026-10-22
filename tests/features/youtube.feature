Feature: YouTube is prepared and does not connect
  The read command stops before any request and does not spawn yt-dlp.
  Nothing here contacts youtube.com.

  Scenario: A channel read is refused
    When I run "--youtube-read alice"
    Then it fails
    And stderr contains "prepared, gated"
    And stderr contains "Nothing was fetched."
    And stderr does not contain "youtube.com"
    And stdout does not contain "alice"

  Scenario: YouTube is listed and starts off
    When I run "--services"
    Then it succeeds
    And stdout contains "redstar  Red Star  enabled"
    And stdout contains "youtube  YouTube  disabled"

  Scenario: Enabling YouTube writes the file and does not fetch
    When I run "--service-enable youtube"
    Then it succeeds
    And stdout contains "youtube enabled"
    And the file ".acp/services/youtube.toml" contains "enabled = true"
