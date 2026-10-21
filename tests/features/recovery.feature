Feature: Recovery phrase and shares
  The phrase and the shares are shown on a terminal, one share at a time.
  A pipe is refused. No fixture in this feature contains a recovery phrase.

  Scenario: A pipe is not given the phrase or the shares
    When I run "--identity-init"
    Then it fails
    And stderr contains "needs a terminal"
    And stdout does not contain "Write these 24 words"
    And stdout does not contain "Share 1 of 5"

  Scenario: Showing recovery again also needs a terminal
    When I run "--show-recovery"
    Then it fails
    And stderr contains "needs a terminal"
    And stdout does not contain "Share 1 of 5"
    And stdout does not contain "Write these 24 words"

  Scenario: Help names the show-again command
    When I run "--help"
    Then it succeeds
    And stdout contains "--show-recovery"
    And stdout contains "Any 3 of those 5"
    And stdout contains "one at a time"

  Scenario: Each share is confirmed before the next is shown
    When the recovery screens walk five shares
    Then the recovery screen showed one share at a time
    And the home has no recovery words on disk
