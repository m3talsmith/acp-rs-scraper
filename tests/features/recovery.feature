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
    And stdout contains "33 words"
    And stdout contains "24 words"

  Scenario: The standard 24-word phrase restores the secret
    When the BIP-39 abandon phrase is restored
    Then stdout contains "phrase words 24"
    And stdout contains "restored 0000000000000000000000000000000000000000000000000000000000000000"

  Scenario: Three official SLIP-39 shares restore the published secret
    When three official SLIP-39 shares are restored
    Then stdout contains "5385577c8cfc6c1a8aa0f7f10ecde0a3318493262591e78b8c14c6686167123b"
    And stdout contains "share words 33"

  Scenario: Product shares of that secret are 33 words and restore it
    When the abandon secret is split and restored from three product shares
    Then stdout contains "product shares 33"
    And stdout contains "phrase words 24"
    And stdout contains "same secret"
